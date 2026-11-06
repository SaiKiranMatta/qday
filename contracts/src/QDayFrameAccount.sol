// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {XMSSVerifier} from "./XMSSVerifier.sol";

/// @notice The staking registry on the devnet.
interface IStakingRegistry {
    function addStake(uint32 unstakeDelay) external payable;
}

/// @notice Receives the account's new root after a rotation, so the account can be found by its current key.
interface IQDayAccountFactory {
    function recordRoot(bytes32 firstRoot, bytes32 firstPubSeed, bytes32 newRoot) external;
}

/// @notice An EIP-8141 account with no ECDSA owner. Only a QDay signature can move its funds.
/// @dev This is the shared implementation. Each account is a minimal proxy (EIP-1167) that delegates
///      here, with its first key appended to its code, so deploying one writes no storage and fits
///      the alt mempool's 500,000 state-gas budget for a deploy frame. Solidity can't emit the frame
///      opcodes, so the implementation deploys six tiny helpers that do.
///      The account's own VERIFY frame may sit anywhere; its spend frame follows it and the action
///      frame follows that. That lets a first transaction put a deploy frame in front.
contract QDayFrameAccount {
    error BadRotation();
    error BadSignature();
    error KeyAlreadyUsed();
    error NoSpendFrame();
    error NotSelf();
    error OpcodeFailed();
    error ReservedKey();

    event Rotated(uint256 tree, bytes32 root, bytes32 pubSeed);

    /// @notice One-time keys per tree.
    uint256 public constant LEAVES = 1024;
    /// @notice Keys at the end of each tree that can only sign rotations.
    uint256 public constant ROTATION_RESERVE = 16;
    /// @notice Length of an encoded signature.
    uint256 public constant SIG_BYTES = 2500;

    bytes32 private constant ROTATE_TAG = "QDAY-ROTATE-V1";

    uint256 private constant MODE_VERIFY = 1;
    uint256 private constant MODE_SENDER = 2;
    uint256 private constant SCHEME_ARBITRARY = 0;
    uint256 private constant APPROVE_SCOPE = 3;
    uint256 private constant SPEND_DATA_BYTES = 36;
    uint256 private constant ROTATE_DATA_BYTES = 68;

    uint256 private constant TX_SIG_HASH = 8;
    uint256 private constant TX_FRAME_COUNT = 9;
    uint256 private constant SIG_SCHEME = 1;
    uint256 private constant SIG_LENGTH = 3;
    uint256 private constant FRAME_TARGET = 0;
    uint256 private constant FRAME_MODE = 2;
    uint256 private constant FRAME_DATA_LENGTH = 4;

    XMSSVerifier public immutable verifier;
    /// @notice The factory that deployed this implementation. Told about rotations.
    address public immutable factory;
    address public immutable approveOp;
    address public immutable txParamOp;
    address public immutable sigParamOp;
    address public immutable sigDataCopyOp;
    address public immutable frameParamOp;
    address public immutable frameDataLoadOp;

    /// @notice Length of the proxy code in front of the first key.
    uint256 public constant PROXY_BYTES = 45;

    /// @dev Set by the first rotation. Until then the key is the one appended to the proxy's code.
    bytes32 private rotatedRoot;
    bytes32 private rotatedPubSeed;
    uint256 public nextIndex;
    uint256 public tree;

    modifier onlySelf() {
        if (msg.sender != address(this)) revert NotSelf();
        _;
    }

    constructor(address verifier_) {
        verifier = XMSSVerifier(verifier_);
        factory = msg.sender;

        approveOp = _deploy(hex"5f355f5faa");
        txParamOp = _deploy(hex"5f35b05f5260205ff3");
        sigParamOp = _deploy(hex"6020355f35b45f5260205ff3");
        sigDataCopyOp = _deploy(hex"6040356020355f355fb56020355ff3");
        frameParamOp = _deploy(hex"6020355f35b35f5260205ff3");
        frameDataLoadOp = _deploy(hex"6020355f35b15f5260205ff3");
    }

    receive() external payable {}

    /// @notice The active tree's root.
    function root() public view returns (bytes32) {
        bytes32 rotated = rotatedRoot;
        if (rotated != bytes32(0)) return rotated;
        (bytes32 first,) = firstKey();
        return first;
    }

    /// @notice The active tree's public seed.
    function pubSeed() public view returns (bytes32) {
        if (rotatedRoot != bytes32(0)) return rotatedPubSeed;
        (, bytes32 first) = firstKey();
        return first;
    }

    /// @notice The key the account was created with, read from the end of its proxy code.
    function firstKey() public view returns (bytes32 firstRoot, bytes32 firstPubSeed) {
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            extcodecopy(address(), ptr, PROXY_BYTES, 0x40)
            firstRoot := mload(ptr)
            firstPubSeed := mload(add(ptr, 0x20))
        }
    }

    /// @notice Approves the transaction if it carries a valid signature. VERIFY frames can't write
    ///         storage, so this only checks that the next frame will spend the key.
    function validate() external {
        if (_sigParam(SIG_SCHEME) != SCHEME_ARBITRARY || _sigParam(SIG_LENGTH) != SIG_BYTES) {
            revert BadSignature();
        }

        bytes memory signature = _signature();
        uint256 index = uint32(bytes4(signature));
        if (index < nextIndex) revert KeyAlreadyUsed();

        uint256 frames = _read(txParamOp, abi.encode(TX_FRAME_COUNT));
        uint256 spendFrame = _ownVerifyFrame(frames) + 1;
        uint256 actionFrame = spendFrame + 1;
        if (frames <= spendFrame || !_isSpendOf(index, spendFrame)) revert NoSpendFrame();

        bytes32 message = bytes32(_read(txParamOp, abi.encode(TX_SIG_HASH)));
        if (frames > actionFrame && _isRotate(actionFrame)) {
            bytes32 newRoot = _frameWord(4, actionFrame);
            bytes32 newPubSeed = _frameWord(36, actionFrame);
            if (newRoot == bytes32(0)) revert BadRotation();
            message = rotationMessage(message, newRoot, newPubSeed);
        } else if (index >= LEAVES - ROTATION_RESERVE) {
            revert ReservedKey();
        }

        if (!verifier.verify(root(), pubSeed(), message, signature)) revert BadSignature();

        (bool ok,) = approveOp.delegatecall(abi.encode(APPROVE_SCOPE));
        if (!ok) revert OpcodeFailed();
    }

    /// @notice Marks every key up to and including `index` as spent.
    function spendIndex(uint256 index) external onlySelf {
        if (index < nextIndex) revert KeyAlreadyUsed();
        nextIndex = index + 1;
    }

    /// @notice Switches to a new tree. The address stays the same.
    function rotate(bytes32 newRoot, bytes32 newPubSeed) external onlySelf {
        if (newRoot == bytes32(0)) revert BadRotation();
        rotatedRoot = newRoot;
        rotatedPubSeed = newPubSeed;
        nextIndex = 0;
        tree += 1;
        emit Rotated(tree, newRoot, newPubSeed);
        (bytes32 firstRoot, bytes32 firstPubSeed) = firstKey();
        IQDayAccountFactory(factory).recordRoot(firstRoot, firstPubSeed, newRoot);
    }

    /// @notice Stakes in a mempool staking registry, from the account's own balance.
    function addStake(address registry, uint256 amount, uint32 unstakeDelay) external onlySelf {
        IStakingRegistry(registry).addStake{value: amount}(unstakeDelay);
    }

    /// @notice The message a rotation signs, which binds the transaction to the next key.
    function rotationMessage(bytes32 sigHash, bytes32 newRoot, bytes32 newPubSeed) public pure returns (bytes32) {
        return keccak256(abi.encode(ROTATE_TAG, sigHash, newRoot, newPubSeed));
    }

    function _deploy(bytes memory runtime) private returns (address helper) {
        bytes memory code = abi.encodePacked(hex"60", uint8(runtime.length), hex"8060095f395ff3", runtime);
        assembly ("memory-safe") {
            helper := create(0, add(code, 0x20), mload(code))
        }
        if (helper == address(0)) revert OpcodeFailed();
    }

    /// @dev The first VERIFY frame: the one running this check. Frames before it can only be a deploy.
    function _ownVerifyFrame(uint256 frames) private view returns (uint256) {
        for (uint256 frame = 0; frame < frames; ++frame) {
            if (_frameParam(frame, FRAME_MODE) == MODE_VERIFY) return frame;
        }
        revert NoSpendFrame();
    }

    function _isSpendOf(uint256 index, uint256 frame) private view returns (bool) {
        return _isSelfCall(frame, SPEND_DATA_BYTES, this.spendIndex.selector) && uint256(_frameWord(4, frame)) == index;
    }

    function _isRotate(uint256 frame) private view returns (bool) {
        return _isSelfCall(frame, ROTATE_DATA_BYTES, this.rotate.selector);
    }

    /// @dev Whether a frame is this account calling itself with the given selector.
    function _isSelfCall(uint256 frame, uint256 dataBytes, bytes4 selector) private view returns (bool) {
        return _frameParam(frame, FRAME_TARGET) == uint256(uint160(address(this)))
            && _frameParam(frame, FRAME_MODE) == MODE_SENDER && _frameParam(frame, FRAME_DATA_LENGTH) == dataBytes
            && bytes4(_frameWord(0, frame)) == selector;
    }

    function _sigParam(uint256 id) private view returns (uint256) {
        return _read(sigParamOp, abi.encode(0, id));
    }

    function _frameParam(uint256 frame, uint256 id) private view returns (uint256) {
        return _read(frameParamOp, abi.encode(frame, id));
    }

    function _frameWord(uint256 offset, uint256 frame) private view returns (bytes32) {
        return bytes32(_read(frameDataLoadOp, abi.encode(offset, frame)));
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
