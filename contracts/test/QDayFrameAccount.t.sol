// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {QDayAccountFactory} from "../src/QDayAccountFactory.sol";
import {QDayFrameAccount} from "../src/QDayFrameAccount.sol";
import {XMSSVerifier} from "../src/XMSSVerifier.sol";
import {
    ApproveStub,
    FrameDataLoadStub,
    FrameParamStub,
    FrameState,
    SigDataCopyStub,
    SigParamStub,
    StakingRegistryStub,
    TxParamStub
} from "./utils/FrameStubs.sol";
import {TestBase} from "./utils/TestBase.sol";

contract QDayFrameAccountTest is TestBase {
    event Rotated(uint256 tree, bytes32 root, bytes32 pubSeed);

    uint256 private constant VERIFY = 1;
    uint256 private constant SENDER = 2;
    uint256 private constant STAKE = 0.012 ether;
    uint32 private constant UNSTAKE_DELAY = 86_400;
    address private constant RECIPIENT = address(0x2222);
    bytes private constant REVERTS = hex"5f5ffd";

    QDayAccountFactory private factory;
    QDayFrameAccount private account;
    FrameState private state;
    StakingRegistryStub private registry;

    bytes32 private root0;
    bytes32 private pubSeed0;
    bytes32 private root1;
    bytes32 private pubSeed1;
    bytes32 private hashA;
    bytes32 private hashB;

    function setUp() public {
        _loadFixtures();
        root0 = _word("root0");
        pubSeed0 = _word("pubSeed0");
        root1 = _word("root1");
        pubSeed1 = _word("pubSeed1");
        hashA = _word("hashA");
        hashB = _word("hashB");

        registry = new StakingRegistryStub();
        factory = new QDayAccountFactory(address(new XMSSVerifier()));
        account = QDayFrameAccount(payable(factory.createAccount(root0, pubSeed0)));
    }

    /// @dev Swaps the helpers for stand-ins, which Foundry's EVM can run.
    function _useStandIns() private {
        state = new FrameState();
        vm.etch(account.approveOp(), address(new ApproveStub(state)).code);
        vm.etch(account.txParamOp(), address(new TxParamStub(state)).code);
        vm.etch(account.sigParamOp(), address(new SigParamStub(state)).code);
        vm.etch(account.sigDataCopyOp(), address(new SigDataCopyStub(state)).code);
        vm.etch(account.frameParamOp(), address(new FrameParamStub(state)).code);
        vm.etch(account.frameDataLoadOp(), address(new FrameDataLoadStub(state)).code);
    }

    function _spendData(uint256 index) private pure returns (bytes memory) {
        return abi.encodeCall(QDayFrameAccount.spendIndex, (index));
    }

    function _rotateData(bytes32 newRoot, bytes32 newPubSeed) private pure returns (bytes memory) {
        return abi.encodeCall(QDayFrameAccount.rotate, (newRoot, newPubSeed));
    }

    /// @dev Sets up frames 0 and 1 of a transaction.
    function _start(bytes32 hash, bytes memory sig, uint256 index) private {
        state.setSignature(hash, 0, sig);
        state.addFrame(address(account), VERIFY, abi.encodeCall(QDayFrameAccount.validate, ()));
        state.addFrame(address(account), SENDER, _spendData(index));
    }

    function _transfer(bytes32 hash, bytes memory sig, uint256 index) private {
        _start(hash, sig, index);
        state.addFrame(RECIPIENT, SENDER, "");
    }

    function _rotation(bytes32 hash, bytes memory sig, uint256 index, bytes32 newRoot, bytes32 newPubSeed) private {
        _start(hash, sig, index);
        state.addFrame(address(account), SENDER, _rotateData(newRoot, newPubSeed));
    }

    function _assertApproved() private view {
        _assertEq(state.approvals(), 1, "not approved once");
        _assertEq(state.approvedScope(), 3, "wrong scope");
        _assertTrue(state.approvedBy() == address(account), "APPROVE was not a delegatecall");
    }

    function _spend(uint256 index) private {
        vm.prank(address(account));
        account.spendIndex(index);
    }

    function _expect(bytes4 selector) private {
        vm.expectRevert(selector);
    }

    // Deployment

    function test_constructor_installs_the_key() public view {
        _assertEq(account.root(), root0, "root");
        _assertEq(account.pubSeed(), pubSeed0, "pubSeed");
        _assertEq(account.nextIndex(), 0, "nextIndex");
        _assertEq(account.tree(), 0, "tree");
        _assertTrue(account.factory() == address(factory), "factory");
        (bytes32 firstRoot, bytes32 firstPubSeed) = account.firstKey();
        _assertEq(firstRoot, root0, "first root");
        _assertEq(firstPubSeed, pubSeed0, "first pubSeed");
    }

    function test_the_account_is_a_small_proxy() public view {
        _assertEq(address(account).code.length, 45 + 64, "proxy plus key");
    }

    function test_the_account_stakes_from_its_own_balance() public {
        vm.deal(address(account), STAKE);
        vm.prank(address(account));
        account.addStake(address(registry), STAKE, UNSTAKE_DELAY);
        _assertTrue(registry.staker() == address(account), "staker");
        _assertEq(registry.staked(), STAKE, "stake");
        _assertEq(uint256(registry.delay()), UNSTAKE_DELAY, "delay");
    }

    function test_only_the_account_can_stake() public {
        _expect(QDayFrameAccount.NotSelf.selector);
        account.addStake(address(registry), 0, UNSTAKE_DELAY);
    }

    function test_a_deploy_frame_in_front_is_allowed() public {
        _useStandIns();
        state.setSignature(hashA, 0, _bytes("sig5"));
        state.addFrame(address(0x4444), 0, hex"deadbeef");
        state.addFrame(address(account), VERIFY, abi.encodeCall(QDayFrameAccount.validate, ()));
        state.addFrame(address(account), SENDER, _spendData(5));
        state.addFrame(RECIPIENT, SENDER, "");
        account.validate();
        _assertApproved();
    }

    function test_a_spend_frame_must_follow_the_verify_frame() public {
        _useStandIns();
        state.setSignature(hashA, 0, _bytes("sig5"));
        state.addFrame(address(account), SENDER, _spendData(5));
        state.addFrame(address(account), VERIFY, abi.encodeCall(QDayFrameAccount.validate, ()));
        _expect(QDayFrameAccount.NoSpendFrame.selector);
        account.validate();
    }

    function test_constants_match_the_scheme() public view {
        _assertEq(account.LEAVES(), 1024, "LEAVES");
        _assertEq(account.ROTATION_RESERVE(), 16, "ROTATION_RESERVE");
        _assertEq(account.SIG_BYTES(), 2500, "SIG_BYTES");
    }

    function test_helpers_are_deployed_with_the_exact_runtime_code() public view {
        _assertEq(keccak256(account.approveOp().code), keccak256(hex"5f355f5faa"), "APPROVE");
        _assertEq(keccak256(account.txParamOp().code), keccak256(hex"5f35b05f5260205ff3"), "TXPARAM");
        _assertEq(keccak256(account.sigParamOp().code), keccak256(hex"6020355f35b45f5260205ff3"), "SIGPARAM");
        _assertEq(
            keccak256(account.sigDataCopyOp().code), keccak256(hex"6040356020355f355fb56020355ff3"), "SIGDATACOPY"
        );
        _assertEq(keccak256(account.frameParamOp().code), keccak256(hex"6020355f35b35f5260205ff3"), "FRAMEPARAM");
        _assertEq(keccak256(account.frameDataLoadOp().code), keccak256(hex"6020355f35b15f5260205ff3"), "FRAMEDATALOAD");
    }

    function test_plain_eth_is_accepted() public {
        (bool ok,) = address(account).call{value: 1}("");
        _assertTrue(ok, "receive refused");
    }

    function test_unknown_calls_revert() public {
        (bool ok,) = address(account).call(hex"deadbeef");
        _assertFalse(ok, "there must be no fallback");
    }

    // validate(): signatures

    function test_a_real_signature_is_approved() public {
        _useStandIns();
        _transfer(hashA, _bytes("sig5"), 5);
        account.validate();
        _assertApproved();
    }

    function test_another_transactions_hash_is_refused() public {
        _useStandIns();
        _transfer(hashB, _bytes("sig5"), 5);
        _expect(QDayFrameAccount.BadSignature.selector);
        account.validate();
    }

    function test_a_tampered_signature_is_refused() public {
        _useStandIns();
        bytes memory sig = _bytes("sig5");
        sig[100] ^= 0x01;
        _transfer(hashA, sig, 5);
        _expect(QDayFrameAccount.BadSignature.selector);
        account.validate();
    }

    function test_a_wrong_length_signature_is_refused() public {
        _useStandIns();
        _transfer(hashA, _bytes("sig5"), 5);
        state.setSigLength(2499);
        _expect(QDayFrameAccount.BadSignature.selector);
        account.validate();
    }

    function test_a_protocol_checked_scheme_is_refused() public {
        _useStandIns();
        _transfer(hashA, _bytes("sig5"), 5);
        state.setSignature(hashA, 1, _bytes("sig5"));
        state.addFrame(address(account), VERIFY, "");
        state.addFrame(address(account), SENDER, _spendData(5));
        _expect(QDayFrameAccount.BadSignature.selector);
        account.validate();
    }

    // validate(): one-time keys

    function test_a_spent_key_is_refused() public {
        _useStandIns();
        _spend(5);
        _transfer(hashA, _bytes("sig5"), 5);
        _expect(QDayFrameAccount.KeyAlreadyUsed.selector);
        account.validate();
    }

    function test_the_next_key_is_approved() public {
        _useStandIns();
        _spend(4);
        _transfer(hashA, _bytes("sig5"), 5);
        account.validate();
        _assertApproved();
    }

    function test_skipping_ahead_is_approved() public {
        _useStandIns();
        _transfer(hashA, _bytes("sig1007"), 1007);
        account.validate();
        _assertApproved();
    }

    // validate(): the spend frame

    function test_a_spend_frame_with_another_index_is_refused() public {
        _useStandIns();
        _transfer(hashA, _bytes("sig5"), 6);
        _expect(QDayFrameAccount.NoSpendFrame.selector);
        account.validate();
    }

    function test_a_spend_frame_with_another_target_is_refused() public {
        _useStandIns();
        state.setSignature(hashA, 0, _bytes("sig5"));
        state.addFrame(address(account), VERIFY, "");
        state.addFrame(RECIPIENT, SENDER, _spendData(5));
        _expect(QDayFrameAccount.NoSpendFrame.selector);
        account.validate();
    }

    function test_a_spend_frame_with_another_mode_is_refused() public {
        _useStandIns();
        state.setSignature(hashA, 0, _bytes("sig5"));
        state.addFrame(address(account), VERIFY, "");
        state.addFrame(address(account), VERIFY, _spendData(5));
        _expect(QDayFrameAccount.NoSpendFrame.selector);
        account.validate();
    }

    function test_a_spend_frame_with_extra_data_is_refused() public {
        _useStandIns();
        state.setSignature(hashA, 0, _bytes("sig5"));
        state.addFrame(address(account), VERIFY, "");
        state.addFrame(address(account), SENDER, abi.encodePacked(_spendData(5), uint8(0)));
        _expect(QDayFrameAccount.NoSpendFrame.selector);
        account.validate();
    }

    function test_a_spend_frame_with_another_selector_is_refused() public {
        _useStandIns();
        state.setSignature(hashA, 0, _bytes("sig5"));
        state.addFrame(address(account), VERIFY, "");
        state.addFrame(address(account), SENDER, abi.encodeWithSelector(0x12345678, uint256(5)));
        _expect(QDayFrameAccount.NoSpendFrame.selector);
        account.validate();
    }

    function test_a_transaction_without_a_spend_frame_is_refused() public {
        _useStandIns();
        state.setSignature(hashA, 0, _bytes("sig5"));
        state.addFrame(address(account), VERIFY, "");
        _expect(QDayFrameAccount.NoSpendFrame.selector);
        account.validate();
    }

    function test_a_transaction_with_only_a_spend_frame_is_approved() public {
        _useStandIns();
        _start(hashA, _bytes("sig5"), 5);
        account.validate();
        _assertApproved();
    }

    // validate(): reserved keys and rotation

    function test_key_1007_still_signs_a_transfer() public {
        _useStandIns();
        _transfer(hashA, _bytes("sig1007"), 1007);
        account.validate();
        _assertApproved();
    }

    function test_reserved_keys_cannot_transfer() public {
        _useStandIns();
        _transfer(hashA, _bytes("sig1008Plain"), 1008);
        _expect(QDayFrameAccount.ReservedKey.selector);
        account.validate();
    }

    function test_reserved_keys_can_rotate() public {
        _useStandIns();
        _rotation(hashA, _bytes("sig1008Rotate"), 1008, root1, pubSeed1);
        account.validate();
        _assertApproved();
    }

    function test_the_last_key_can_rotate() public {
        _useStandIns();
        _rotation(hashA, _bytes("sig1023Rotate"), 1023, root1, pubSeed1);
        account.validate();
        _assertApproved();
    }

    function test_an_ordinary_key_can_rotate() public {
        _useStandIns();
        _rotation(hashA, _bytes("sig5Rotate"), 5, root1, pubSeed1);
        account.validate();
        _assertApproved();
    }

    function test_a_rotation_over_the_plain_hash_is_refused() public {
        _useStandIns();
        _rotation(hashA, _bytes("sig1008Plain"), 1008, root1, pubSeed1);
        _expect(QDayFrameAccount.BadSignature.selector);
        account.validate();
    }

    function test_a_swapped_next_key_is_refused() public {
        _useStandIns();
        _rotation(hashA, _bytes("sig1008Rotate"), 1008, root0, pubSeed0);
        _expect(QDayFrameAccount.BadSignature.selector);
        account.validate();
    }

    function test_a_swapped_next_pub_seed_is_refused() public {
        _useStandIns();
        _rotation(hashA, _bytes("sig1008Rotate"), 1008, root1, pubSeed0);
        _expect(QDayFrameAccount.BadSignature.selector);
        account.validate();
    }

    function test_a_rotation_to_an_empty_root_is_refused() public {
        _useStandIns();
        _rotation(hashA, _bytes("sig1008Rotate"), 1008, bytes32(0), pubSeed1);
        _expect(QDayFrameAccount.BadRotation.selector);
        account.validate();
    }

    function test_a_rotate_frame_with_another_mode_is_not_a_rotation() public {
        _useStandIns();
        _start(hashA, _bytes("sig1008Rotate"), 1008);
        state.addFrame(address(account), VERIFY, _rotateData(root1, pubSeed1));
        _expect(QDayFrameAccount.ReservedKey.selector);
        account.validate();
    }

    function test_a_rotate_frame_with_another_target_is_not_a_rotation() public {
        _useStandIns();
        _start(hashA, _bytes("sig1008Rotate"), 1008);
        state.addFrame(RECIPIENT, SENDER, _rotateData(root1, pubSeed1));
        _expect(QDayFrameAccount.ReservedKey.selector);
        account.validate();
    }

    function test_only_the_new_tree_signs_after_a_rotation() public {
        _useStandIns();
        _rotation(hashA, _bytes("sig1008Rotate"), 1008, root1, pubSeed1);
        account.validate();
        _spend(1008);
        vm.prank(address(account));
        account.rotate(root1, pubSeed1);

        _transfer(hashB, _bytes("sig6HashB"), 6);
        _expect(QDayFrameAccount.BadSignature.selector);
        account.validate();

        _transfer(hashB, _bytes("newSig0HashB"), 0);
        account.validate();
        _assertApproved();
    }

    // spendIndex()

    function test_spend_index_is_self_only() public {
        _expect(QDayFrameAccount.NotSelf.selector);
        account.spendIndex(0);
    }

    function test_spend_index_moves_next_index_past_the_key() public {
        _spend(5);
        _assertEq(account.nextIndex(), 6, "nextIndex");
    }

    function test_spend_index_refuses_a_spent_key() public {
        _spend(5);
        _expect(QDayFrameAccount.KeyAlreadyUsed.selector);
        _spend(5);
    }

    // rotate()

    function test_rotate_is_self_only() public {
        _expect(QDayFrameAccount.NotSelf.selector);
        account.rotate(root1, pubSeed1);
    }

    function test_rotate_refuses_an_empty_root() public {
        vm.prank(address(account));
        _expect(QDayFrameAccount.BadRotation.selector);
        account.rotate(bytes32(0), pubSeed1);
    }

    function test_rotate_installs_the_key_and_resets_the_index() public {
        _spend(1008);
        vm.expectEmit();
        emit Rotated(1, root1, pubSeed1);
        vm.prank(address(account));
        account.rotate(root1, pubSeed1);

        _assertEq(account.root(), root1, "root");
        _assertEq(account.pubSeed(), pubSeed1, "pubSeed");
        _assertEq(account.nextIndex(), 0, "nextIndex");
        _assertEq(account.tree(), 1, "tree");
    }

    function test_rotation_message_matches_the_published_vector() public view {
        bytes32 got =
            account.rotationMessage(0xa7c6c4859d6884586888ef4a36d41a909f04e0ecbc2a8110151bf4a64ffcae7a, root1, pubSeed1);
        _assertEq(got, 0xa332fb67be17639d700170f0e46f80323df4a5f50e9f9101b43569f9aa02de2a, "message");
    }

    // Helper failures

    function test_a_failed_helper_call_reverts_opcode_failed() public {
        address[6] memory helpers = [
            account.txParamOp(),
            account.sigParamOp(),
            account.sigDataCopyOp(),
            account.frameParamOp(),
            account.frameDataLoadOp(),
            account.approveOp()
        ];
        for (uint256 i = 0; i < helpers.length; i++) {
            _useStandIns();
            _transfer(hashA, _bytes("sig5"), 5);
            vm.etch(helpers[i], REVERTS);
            _expect(QDayFrameAccount.OpcodeFailed.selector);
            account.validate();
        }
    }
}
