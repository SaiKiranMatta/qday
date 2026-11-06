// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {IMLDSAInlineVerifier} from "./IMLDSAInlineVerifier.sol";

/// @notice Holds stealth payments to one-time ML-DSA keys, which a sender derives from the
///         recipient's published key and an X-Wing shared secret. Nothing is deployed per payment:
///         the key arrives in calldata when it's spent, and every key shares one matrix.
/// @dev The one-time keys use a signer variant that hasn't been reviewed, so this isn't for real
///      funds. The contract can't check a key, so a recipient must only accept a payment whose key
///      hash they can derive themselves. A spend signs for the whole note rather than an amount, so
///      a top-up can't invalidate it, and a failed payout is kept for the payee to withdraw.
contract QDayLatticePool {
    error AlreadySpent();
    error BadCiphertext();
    error BadCodeHash();
    error BadKey();
    error BadSignature();
    error NoNote();
    error NothingOwed();
    error NothingPaid();
    error WithdrawFailed();

    event Announcement(bytes32 indexed keyHash, bytes1 viewTag, bytes ciphertext);
    event Spent(bytes32 indexed keyHash, address indexed to, uint256 amount, uint256 fee);
    event Kept(address indexed payee, uint256 amount);

    /// @notice Length of a key: `tr`, then `t1` in the NTT domain.
    uint256 public constant KEY_BYTES = 4160;
    /// @notice Length of an X-Wing ciphertext.
    uint256 public constant CIPHERTEXT_BYTES = 1120;
    /// @notice A note's balance once it's spent.
    uint256 public constant SPENT = type(uint256).max;

    /// @dev Enough for a smart account's receive function, and it caps what a payee can burn.
    uint256 private constant PAYOUT_GAS = 100_000;

    IMLDSAInlineVerifier public immutable verifier;
    bytes32 public immutable verifierCodeHash;
    address public immutable matrix;
    bytes32 public immutable matrixCodeHash;

    /// @notice The balance held for each key's hash.
    mapping(bytes32 keyHash => uint256 balance) public notes;
    /// @notice What failed payouts left for each payee to withdraw.
    mapping(address payee => uint256 amount) public owed;

    constructor(IMLDSAInlineVerifier verifier_, address matrix_) {
        verifier = verifier_;
        verifierCodeHash = address(verifier_).codehash;
        matrix = matrix_;
        matrixCodeHash = matrix_.codehash;
    }

    /// @notice Pays a note and announces it, so the recipient can find it by scanning.
    function pay(bytes32 keyHash, bytes1 viewTag, bytes calldata ciphertext) external payable {
        if (msg.value == 0) revert NothingPaid();
        if (ciphertext.length != CIPHERTEXT_BYTES) revert BadCiphertext();
        uint256 balance = notes[keyHash];
        if (balance == SPENT) revert AlreadySpent();
        notes[keyHash] = balance + msg.value;
        emit Announcement(keyHash, viewTag, ciphertext);
    }

    /// @notice What a spend signs.
    function spendHash(bytes32 keyHash, address to, uint256 fee) public view returns (bytes32) {
        return keccak256(abi.encode(block.chainid, address(this), keyHash, to, fee));
    }

    /// @notice Sends a whole note to `to`. Anyone may submit this, and `fee` repays them.
    function spend(bytes calldata key, bytes calldata signature, address to, uint256 fee) external {
        if (key.length != KEY_BYTES) revert BadKey();
        bytes32 keyHash = keccak256(key);
        uint256 balance = notes[keyHash];
        if (balance == SPENT) revert AlreadySpent();
        if (balance == 0) revert NoNote();
        if (address(verifier).codehash != verifierCodeHash || matrix.codehash != matrixCodeHash) {
            revert BadCodeHash();
        }

        // The verifier reverts on some bad input. That means not accepted, the same as false.
        bytes memory message = abi.encodePacked(spendHash(keyHash, to, fee));
        try verifier.verifyInline(matrix, key, message, signature) returns (bool accepted) {
            if (!accepted) revert BadSignature();
        } catch {
            revert BadSignature();
        }

        notes[keyHash] = SPENT;
        uint256 paidFee = fee < balance ? fee : balance;
        uint256 amount = balance - paidFee;
        emit Spent(keyHash, to, amount, paidFee);
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
}
