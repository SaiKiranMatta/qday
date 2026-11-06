// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

/// @notice Holds payments to one-time hash keys. The recipient gives the sender a fresh key for each
///         payment, so nothing on chain links two payments, and nothing is deployed for any of them.
/// @dev A one-time key must never sign two different messages. So once a valid signature arrives,
///      this always finishes: it signs for the whole note rather than an amount, so a top-up can't
///      invalidate it, and a failed payout is kept for the payee to withdraw rather than reverted.
contract QDayHashPool {
    error AlreadySpent();
    error BadSignature();
    error NoNote();
    error NothingOwed();
    error NothingPaid();
    error WithdrawFailed();

    event Paid(bytes32 indexed leaf, uint256 value);
    event Spent(bytes32 indexed leaf, address indexed to, uint256 amount, uint256 fee);
    event Kept(address indexed payee, uint256 amount);

    /// @notice Length of an encoded one-time signature.
    uint256 public constant SIG_BYTES = 2176;
    /// @notice A note's balance once it's spent. A spent key can't take another payment.
    uint256 public constant SPENT = type(uint256).max;

    /// @dev Enough for a smart account's receive function, and it caps what a payee can burn.
    uint256 private constant PAYOUT_GAS = 100_000;

    /// @notice The balance held for each key's leaf.
    mapping(bytes32 leaf => uint256 balance) public notes;
    /// @notice What failed payouts left for each payee to withdraw.
    mapping(address payee => uint256 amount) public owed;

    /// @notice Pays a note. Paying a note twice adds to it.
    function pay(bytes32 leaf) external payable {
        if (msg.value == 0) revert NothingPaid();
        uint256 balance = notes[leaf];
        if (balance == SPENT) revert AlreadySpent();
        notes[leaf] = balance + msg.value;
        emit Paid(leaf, msg.value);
    }

    /// @notice What a spend signs.
    function spendHash(bytes32 leaf, address to, uint256 fee) public view returns (bytes32) {
        return keccak256(abi.encode(block.chainid, address(this), leaf, to, fee));
    }

    /// @notice Sends a whole note to `to`. Anyone may submit this, and `fee` repays them.
    /// @dev The fee is capped at the balance rather than refused, because a revert here would leave
    ///      a published signature for a key that can't safely sign again.
    function spend(bytes32 leaf, bytes32 pubSeed, bytes calldata signature, address to, uint256 fee) external {
        uint256 balance = notes[leaf];
        if (balance == SPENT) revert AlreadySpent();
        if (balance == 0) revert NoNote();
        if (signature.length != SIG_BYTES) revert BadSignature();
        if (_recoverLeaf(leaf, pubSeed, spendHash(leaf, to, fee), signature) != leaf) revert BadSignature();

        notes[leaf] = SPENT;
        uint256 paidFee = fee < balance ? fee : balance;
        uint256 amount = balance - paidFee;
        emit Spent(leaf, to, amount, paidFee);
        _payOut(to, amount);
        _payOut(msg.sender, paidFee);
    }

    /// @notice Withdraws what a failed payout left.
    function withdraw() external {
        uint256 amount = owed[msg.sender];
        if (amount == 0) revert NothingOwed();
        owed[msg.sender] = 0;
        (bool paid,) = msg.sender.call{value: amount}("");
        if (!paid) revert WithdrawFailed();
    }

    /// @dev Never reverts, and doesn't copy what the payee returns.
    function _payOut(address payee, uint256 amount) private {
        if (amount == 0) return;
        bool paid;
        assembly ("memory-safe") {
            paid := call(PAYOUT_GAS, payee, amount, 0, 0, 0, 0)
        }
        if (!paid) {
            owed[payee] += amount;
            emit Kept(payee, amount);
        }
    }

    /// @dev The chain and leaf hashes of `XMSSVerifier`, stopping at the leaf. A one-time key is key
    ///      0, so its address bits are zero. The work area is above the free memory pointer.
    function _recoverLeaf(bytes32 leaf, bytes32 pubSeed, bytes32 message, bytes calldata signature)
        private
        pure
        returns (bytes32 recovered)
    {
        assembly ("memory-safe") {
            /// Finishes chain `chain` from step `digit` and stores its end for the leaf hash.
            function finishChain(work, chain, digit, chainsAt) {
                let position := shl(5, chain)
                let value := add(work, 0x40)
                mstore(value, calldataload(add(chainsAt, position)))
                let chainBits := shl(32, chain)
                let last := or(chainBits, 15)
                for { let adrs := or(chainBits, digit) } lt(adrs, last) { adrs := add(adrs, 1) } {
                    mstore(add(work, 0x20), adrs)
                    mstore(value, keccak256(work, 0x60))
                }
                mstore(add(add(work, 0xc0), position), mload(value))
            }

            let work := mload(0x40)

            // digest = H(r || leaf || u256(0) || message)
            mstore(work, calldataload(signature.offset))
            mstore(add(work, 0x20), leaf)
            mstore(add(work, 0x40), 0)
            mstore(add(work, 0x60), message)
            let digest := keccak256(work, 0x80)

            // Chain hashes are H(pubSeed || ADRS || value), and CHAIN has type 0.
            mstore(work, pubSeed)
            let chainsAt := add(signature.offset, 32)
            let checksum := 960
            for { let chain := 0 } lt(chain, 64) { chain := add(chain, 1) } {
                let digit := shr(252, digest)
                digest := shl(4, digest)
                checksum := sub(checksum, digit)
                finishChain(work, chain, digit, chainsAt)
            }
            finishChain(work, 64, shr(8, checksum), chainsAt)
            finishChain(work, 65, and(shr(4, checksum), 15), chainsAt)
            finishChain(work, 66, and(checksum, 15), chainsAt)

            // leaf = H(pubSeed || ADRS(LEAF) || 67 chain ends)
            mstore(add(work, 0x80), pubSeed)
            mstore(add(work, 0xa0), shl(248, 1))
            recovered := keccak256(add(work, 0x80), 2208)
        }
    }
}
