// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {IMLDSAVerifier} from "./IMLDSAVerifier.sol";
import {IStakingRegistry} from "./QDayFrameAccount.sol";

/// @notice An EIP-8141 account with no ECDSA owner. Its key is a prepared ML-DSA-44 public key
///         blob, and it doesn't care how that key was made.
/// @dev The blob must be validated off-chain before deployment: the verifier can't check it. The
///      blob and the verifier are bound by code hash and re-checked at every use, so what was
///      validated is what runs. Keys can't be used up, so there's no key index and no rotation.
contract QDayMlDsaAccount {
    error BadBlob();
    error BadCodeHash();
    error BadSignature();
    error OpcodeFailed();

    /// @notice Length of an encoded signature.
    uint256 public constant SIG_BYTES = 2420;
    /// @notice Length of a prepared public key blob.
    uint256 public constant BLOB_BYTES = 20_545;

    uint256 private constant SCHEME_ARBITRARY = 0;
    uint256 private constant APPROVE_SCOPE = 3;
    uint256 private constant TX_SIG_HASH = 8;
    uint256 private constant SIG_SCHEME = 1;
    uint256 private constant SIG_LENGTH = 3;

    IMLDSAVerifier public immutable verifier;
    bytes32 public immutable verifierCodeHash;
    address public immutable pkBlob;
    bytes32 public immutable pkBlobHash;
    address public immutable approveOp;
    address public immutable txParamOp;
    address public immutable sigParamOp;
    address public immutable sigDataCopyOp;

    /// @param stakingRegistry The registry to stake with, or zero for no stake.
    constructor(
        address verifier_,
        bytes32 verifierCodeHash_,
        address pkBlob_,
        bytes32 pkBlobHash_,
        address stakingRegistry,
        uint256 stake,
        uint32 unstakeDelay
    ) payable {
        if (verifier_.codehash != verifierCodeHash_ || pkBlob_.codehash != pkBlobHash_) {
            revert BadCodeHash();
        }
        if (pkBlob_.code.length != BLOB_BYTES) revert BadBlob();

        verifier = IMLDSAVerifier(verifier_);
        verifierCodeHash = verifierCodeHash_;
        pkBlob = pkBlob_;
        pkBlobHash = pkBlobHash_;

        approveOp = _deploy(hex"5f355f5faa");
        txParamOp = _deploy(hex"5f35b05f5260205ff3");
        sigParamOp = _deploy(hex"6020355f35b45f5260205ff3");
        sigDataCopyOp = _deploy(hex"6040356020355f355fb56020355ff3");

        if (stakingRegistry != address(0)) {
            IStakingRegistry(stakingRegistry).addStake{value: stake}(unstakeDelay);
        }
    }

    receive() external payable {}

    /// @notice Approves the transaction if it carries a valid signature over its hash.
    function validate() external {
        if (_sigParam(SIG_SCHEME) != SCHEME_ARBITRARY || _sigParam(SIG_LENGTH) != SIG_BYTES) {
            revert BadSignature();
        }
        if (address(verifier).codehash != verifierCodeHash || pkBlob.codehash != pkBlobHash) {
            revert BadCodeHash();
        }

        bytes memory signature = _signature();
        bytes32 sigHash = bytes32(_read(txParamOp, abi.encode(TX_SIG_HASH)));

        // The verifier reverts on some bad input. That means not accepted, the same as false.
        try verifier.verify(pkBlob, abi.encodePacked(sigHash), signature) returns (bool accepted) {
            if (!accepted) revert BadSignature();
        } catch {
            revert BadSignature();
        }

        (bool ok,) = approveOp.delegatecall(abi.encode(APPROVE_SCOPE));
        if (!ok) revert OpcodeFailed();
    }

    function _deploy(bytes memory runtime) private returns (address helper) {
        bytes memory code = abi.encodePacked(hex"60", uint8(runtime.length), hex"8060095f395ff3", runtime);
        assembly ("memory-safe") {
            helper := create(0, add(code, 0x20), mload(code))
        }
        if (helper == address(0)) revert OpcodeFailed();
    }

    function _sigParam(uint256 id) private view returns (uint256) {
        return _read(sigParamOp, abi.encode(0, id));
    }

    function _signature() private view returns (bytes memory signature) {
        bool ok;
        (ok, signature) = sigDataCopyOp.staticcall(abi.encode(0, SIG_BYTES, 0));
        if (!ok || signature.length != SIG_BYTES) revert OpcodeFailed();
    }

    function _read(address helper, bytes memory input) private view returns (uint256) {
        (bool ok, bytes memory output) = helper.staticcall(input);
        if (!ok || output.length != 32) revert OpcodeFailed();
        return uint256(bytes32(output));
    }
}
