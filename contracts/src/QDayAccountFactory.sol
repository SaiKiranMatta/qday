// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {QDayFrameAccount} from "./QDayFrameAccount.sol";

/// @notice Deploys QDay accounts at addresses that follow from their first key, so a wallet can show
///         the address (and receive funds there) before the account exists. An account's first
///         transaction deploys it through `createAccount` in a deploy frame and pays for itself.
/// @dev Each account is an EIP-1167 proxy to one shared implementation, with its first key appended
///      to the proxy code. Creating one writes no storage, which keeps the deploy frame under the alt
///      mempool's 500,000 state-gas limit.
contract QDayAccountFactory {
    event AccountCreated(address indexed account, bytes32 root, bytes32 pubSeed);
    event RootRecorded(address indexed account, bytes32 root);

    /// @notice The code every account delegates to.
    QDayFrameAccount public immutable implementation;

    /// @notice Accounts found by a root they rotated to. First roots need no entry: use `accountAddress`.
    mapping(bytes32 root => address account) public rotatedAccountOf;

    constructor(address verifier) {
        implementation = new QDayFrameAccount(verifier);
    }

    /// @notice Deploys the account for a first key. Fails if it already exists.
    /// @dev No "already deployed" check: the alt mempool forbids a factory from reading the code of the
    ///      account it is about to create.
    function createAccount(bytes32 root, bytes32 pubSeed) external returns (address account) {
        bytes memory code = _creationCode(root, pubSeed);
        bytes32 salt = _salt(root, pubSeed);
        assembly ("memory-safe") {
            account := create2(0, add(code, 0x20), mload(code), salt)
        }
        require(account != address(0), "create failed");
        emit AccountCreated(account, root, pubSeed);
    }

    /// @notice Where the account for a first key lives, deployed or not.
    function accountAddress(bytes32 root, bytes32 pubSeed) public view returns (address) {
        bytes32 hash = keccak256(
            abi.encodePacked(bytes1(0xff), address(this), _salt(root, pubSeed), keccak256(_creationCode(root, pubSeed)))
        );
        return address(uint160(uint256(hash)));
    }

    /// @notice The account behind any root it has used: rotated roots by lookup, first roots by address.
    function accountOf(bytes32 root, bytes32 pubSeed) external view returns (address) {
        address rotated = rotatedAccountOf[root];
        return rotated != address(0) ? rotated : accountAddress(root, pubSeed);
    }

    /// @notice Called by an account after it rotates, so it can still be found by its current key.
    function recordRoot(bytes32 firstRoot, bytes32 firstPubSeed, bytes32 newRoot) external {
        require(msg.sender == accountAddress(firstRoot, firstPubSeed), "not a QDay account");
        rotatedAccountOf[newRoot] = msg.sender;
        emit RootRecorded(msg.sender, newRoot);
    }

    function _salt(bytes32 root, bytes32 pubSeed) private pure returns (bytes32) {
        return keccak256(abi.encode(root, pubSeed));
    }

    /// @dev EIP-1167 proxy runtime (45 bytes), then the 64-byte first key, behind a constructor that
    ///      returns all 109 bytes as the account's code.
    function _creationCode(bytes32 root, bytes32 pubSeed) private view returns (bytes memory) {
        return abi.encodePacked(
            hex"3d606d80600a3d3981f3",
            hex"363d3d373d3d3d363d73",
            address(implementation),
            hex"5af43d82803e903d91602b57fd5bf3",
            root,
            pubSeed
        );
    }
}
