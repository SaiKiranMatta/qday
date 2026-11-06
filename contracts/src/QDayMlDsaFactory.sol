// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {QDayMlDsaAccount} from "./QDayMlDsaAccount.sol";

/// @notice Deploys blobs and accounts at addresses known in advance, so an account can receive
///         funds before it exists.
contract QDayMlDsaFactory {
    error BadBlob();
    error DeployFailed();

    uint256 private constant BLOB_BYTES = 20_545;
    bytes32 private constant BLOB_SALT = 0;

    /// @notice Where `blob` will be deployed.
    function predictBlob(bytes calldata blob) public view returns (address) {
        return _predict(BLOB_SALT, keccak256(_blobInitcode(blob)));
    }

    /// @notice Deploys a blob as a plain data contract. The blob must be validated off-chain first.
    function deployBlob(bytes calldata blob) external returns (address at) {
        // EIP-3541 forbids code starting with 0xEF, and every prepared blob starts with 0x00.
        if (blob.length != BLOB_BYTES || blob[0] != 0x00) revert BadBlob();
        at = predictBlob(blob);
        if (at.code.length != 0) return at;
        if (_create(BLOB_SALT, 0, _blobInitcode(blob)) != at) revert DeployFailed();
    }

    /// @notice Where an account with these settings and `salt` will be deployed.
    function predictAccount(
        address verifier,
        bytes32 verifierCodeHash,
        address pkBlob,
        bytes32 pkBlobHash,
        address stakingRegistry,
        uint256 stake,
        uint32 unstakeDelay,
        bytes32 salt
    ) public view returns (address) {
        bytes memory code =
            _accountInitcode(verifier, verifierCodeHash, pkBlob, pkBlobHash, stakingRegistry, stake, unstakeDelay);
        return _predict(salt, keccak256(code));
    }

    /// @notice Deploys an account. Any value sent goes to it, and pays its stake.
    function deployAccount(
        address verifier,
        bytes32 verifierCodeHash,
        address pkBlob,
        bytes32 pkBlobHash,
        address stakingRegistry,
        uint256 stake,
        uint32 unstakeDelay,
        bytes32 salt
    ) external payable returns (address at) {
        bytes memory code =
            _accountInitcode(verifier, verifierCodeHash, pkBlob, pkBlobHash, stakingRegistry, stake, unstakeDelay);
        at = _create(salt, msg.value, code);
        if (at == address(0)) revert DeployFailed();
    }

    function _accountInitcode(
        address verifier,
        bytes32 verifierCodeHash,
        address pkBlob,
        bytes32 pkBlobHash,
        address stakingRegistry,
        uint256 stake,
        uint32 unstakeDelay
    ) private pure returns (bytes memory) {
        return abi.encodePacked(
            type(QDayMlDsaAccount).creationCode,
            abi.encode(verifier, verifierCodeHash, pkBlob, pkBlobHash, stakingRegistry, stake, unstakeDelay)
        );
    }

    /// @dev The stub copies the blob out and returns it, so the code is exactly the blob.
    function _blobInitcode(bytes calldata blob) private pure returns (bytes memory) {
        return abi.encodePacked(
            bytes1(0x61), uint16(blob.length), hex"600e600039", bytes1(0x61), uint16(blob.length), hex"6000f3", blob
        );
    }

    function _create(bytes32 salt, uint256 value, bytes memory code) private returns (address at) {
        assembly ("memory-safe") {
            at := create2(value, add(code, 0x20), mload(code), salt)
        }
    }

    function _predict(bytes32 salt, bytes32 codeHash) private view returns (address) {
        bytes32 digest = keccak256(abi.encodePacked(bytes1(0xff), address(this), salt, codeHash));
        return address(uint160(uint256(digest)));
    }
}
