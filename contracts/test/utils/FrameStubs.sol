// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

/// @notice The transaction a stand-in reports, and a record of what APPROVE was asked to do.
contract FrameState {
    struct Frame {
        address target;
        uint256 mode;
        bytes data;
    }

    bytes32 public sigHash;
    uint256 public scheme;
    uint256 public sigLength;
    bytes public signature;
    Frame[] private frames;

    uint256 public approvals;
    uint256 public approvedScope;
    address public approvedBy;

    function setSignature(bytes32 hash, uint256 newScheme, bytes memory sig) external {
        sigHash = hash;
        scheme = newScheme;
        sigLength = sig.length;
        signature = sig;
        delete frames;
        approvals = 0;
        approvedScope = 0;
        approvedBy = address(0);
    }

    function setSigLength(uint256 length) external {
        sigLength = length;
    }

    function addFrame(address frameTarget, uint256 frameMode, bytes memory data) external {
        frames.push(Frame(frameTarget, frameMode, data));
    }

    function recordApproval(uint256 scope) external {
        approvals += 1;
        approvedScope = scope;
        approvedBy = msg.sender;
    }

    function frameCount() external view returns (uint256) {
        return frames.length;
    }

    function target(uint256 frame) external view returns (address) {
        return frames[frame].target;
    }

    function mode(uint256 frame) external view returns (uint256) {
        return frames[frame].mode;
    }

    function dataLength(uint256 frame) external view returns (uint256) {
        return frames[frame].data.length;
    }

    /// @dev Reads past the end as zeros, as CALLDATALOAD does.
    function dataWord(uint256 offset, uint256 frame) external view returns (bytes32 word) {
        bytes memory data = frames[frame].data;
        for (uint256 i = 0; i < 32 && offset + i < data.length; i++) {
            word |= bytes32(data[offset + i]) >> (8 * i);
        }
    }
}

/// @dev Stand-ins take the same input as the real helpers and fail on any other.
contract ApproveStub {
    FrameState private immutable state;

    constructor(FrameState frameState) {
        state = frameState;
    }

    fallback(bytes calldata input) external returns (bytes memory) {
        require(input.length == 32, "approve input");
        state.recordApproval(abi.decode(input, (uint256)));
        return "";
    }
}

contract TxParamStub {
    FrameState private immutable state;

    constructor(FrameState frameState) {
        state = frameState;
    }

    fallback(bytes calldata input) external returns (bytes memory) {
        require(input.length == 32, "txparam input");
        uint256 id = abi.decode(input, (uint256));
        if (id == 8) return abi.encode(state.sigHash());
        if (id == 9) return abi.encode(state.frameCount());
        revert("txparam id");
    }
}

contract SigParamStub {
    FrameState private immutable state;

    constructor(FrameState frameState) {
        state = frameState;
    }

    fallback(bytes calldata input) external returns (bytes memory) {
        require(input.length == 64, "sigparam input");
        (uint256 entry, uint256 id) = abi.decode(input, (uint256, uint256));
        require(entry == 0, "sigparam entry");
        if (id == 1) return abi.encode(state.scheme());
        if (id == 3) return abi.encode(state.sigLength());
        revert("sigparam id");
    }
}

contract SigDataCopyStub {
    FrameState private immutable state;

    constructor(FrameState frameState) {
        state = frameState;
    }

    fallback(bytes calldata input) external returns (bytes memory) {
        require(input.length == 96, "sigdatacopy input");
        (uint256 entry, uint256 length, uint256 offset) = abi.decode(input, (uint256, uint256, uint256));
        require(entry == 0 && (length == 2500 || length == 2420) && offset == 0, "sigdatacopy args");
        bytes memory out = new bytes(length);
        bytes memory sig = state.signature();
        for (uint256 i = 0; i < length && i < sig.length; i++) {
            out[i] = sig[i];
        }
        return out;
    }
}

contract FrameParamStub {
    FrameState private immutable state;

    constructor(FrameState frameState) {
        state = frameState;
    }

    fallback(bytes calldata input) external returns (bytes memory) {
        require(input.length == 64, "frameparam input");
        (uint256 frame, uint256 id) = abi.decode(input, (uint256, uint256));
        if (id == 0) return abi.encode(state.target(frame));
        if (id == 2) return abi.encode(state.mode(frame));
        if (id == 4) return abi.encode(state.dataLength(frame));
        revert("frameparam id");
    }
}

contract FrameDataLoadStub {
    FrameState private immutable state;

    constructor(FrameState frameState) {
        state = frameState;
    }

    fallback(bytes calldata input) external returns (bytes memory) {
        require(input.length == 64, "framedataload input");
        (uint256 offset, uint256 frame) = abi.decode(input, (uint256, uint256));
        return abi.encode(state.dataWord(offset, frame));
    }
}

/// @notice Records the stake passed to it by the account's constructor.
contract StakingRegistryStub {
    address public staker;
    uint256 public staked;
    uint32 public delay;

    function addStake(uint32 unstakeDelay) external payable {
        staker = msg.sender;
        staked = msg.value;
        delay = unstakeDelay;
    }
}
