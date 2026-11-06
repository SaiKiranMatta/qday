// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {IMLDSAVerifier} from "../src/IMLDSAVerifier.sol";
import {QDayMlDsaAccount} from "../src/QDayMlDsaAccount.sol";
import {QDayMlDsaFactory} from "../src/QDayMlDsaFactory.sol";
import {
    ApproveStub,
    FrameState,
    SigDataCopyStub,
    SigParamStub,
    StakingRegistryStub,
    TxParamStub
} from "./utils/FrameStubs.sol";
import {TestBase} from "./utils/TestBase.sol";

/// Deploys the real verifier and real blobs, and checks real signatures from the Rust signer.
abstract contract MlDsaBase is TestBase {
    event log_named_uint(string key, uint256 val);

    uint256 internal constant STAKE = 0.012 ether;
    uint32 internal constant UNSTAKE_DELAY = 86_400;
    bytes internal constant REVERTS = hex"5f5ffd";
    address internal constant RECIPIENT = address(0x2222);

    IMLDSAVerifier internal verifier;
    address internal verifierHelper;
    QDayMlDsaFactory internal factory;
    FrameState internal state;
    StakingRegistryStub internal registry;
    bytes32 internal verifierHash;
    bytes internal blob;
    bytes32 internal hashA;
    bytes32 internal hashB;
    address internal pkBlob;
    bytes32 internal blobHash;

    function _deployRaw(bytes memory runtime) internal returns (address at) {
        bytes memory code = abi.encodePacked(
            bytes1(0x61),
            uint16(runtime.length),
            hex"600e600039",
            bytes1(0x61),
            uint16(runtime.length),
            hex"6000f3",
            runtime
        );
        assembly {
            at := create(0, add(code, 32), mload(code))
        }
        require(at != address(0), "raw deploy failed");
    }

    function _deployVerifier() internal returns (IMLDSAVerifier deployed) {
        verifierHelper = _deployRaw(_bytes("helperRuntime"));
        bytes memory code = abi.encodePacked(_bytes("verifierInitcode"), abi.encode(verifierHelper));
        address at;
        assembly {
            at := create(0, add(code, 32), mload(code))
        }
        require(at != address(0), "verifier deploy failed");
        deployed = IMLDSAVerifier(at);
    }

    function _setUpMlDsa() internal {
        _loadFixtures("test/fixtures/mldsa.json");
        verifier = _deployVerifier();
        verifierHash = address(verifier).codehash;
        registry = new StakingRegistryStub();
        factory = new QDayMlDsaFactory();
        blob = _bytes("blob");
        pkBlob = _deployRaw(blob);
        blobHash = pkBlob.codehash;
        hashA = _word("hashA");
        hashB = _word("hashB");
    }

    function _newAccount(address stakingRegistry, uint256 stake) internal returns (QDayMlDsaAccount) {
        return new QDayMlDsaAccount{value: stake}(
            address(verifier), verifierHash, pkBlob, blobHash, stakingRegistry, stake, UNSTAKE_DELAY
        );
    }

    function _useStandIns(QDayMlDsaAccount account) internal {
        state = new FrameState();
        vm.etch(account.approveOp(), address(new ApproveStub(state)).code);
        vm.etch(account.txParamOp(), address(new TxParamStub(state)).code);
        vm.etch(account.sigParamOp(), address(new SigParamStub(state)).code);
        vm.etch(account.sigDataCopyOp(), address(new SigDataCopyStub(state)).code);
    }

    function _transaction(QDayMlDsaAccount account, bytes32 hash, bytes memory sig) internal {
        state.setSignature(hash, 0, sig);
        state.addFrame(address(account), 1, abi.encodeCall(QDayMlDsaAccount.validate, ()));
        state.addFrame(RECIPIENT, 2, "");
    }

    function _assertApproved(QDayMlDsaAccount account) internal view {
        _assertEq(state.approvals(), 1, "not approved once");
        _assertEq(state.approvedScope(), 3, "wrong scope");
        _assertTrue(state.approvedBy() == address(account), "APPROVE was not a delegatecall");
    }
}

contract QDayMlDsaAccountTest is MlDsaBase {
    QDayMlDsaAccount private account;

    function setUp() public {
        _setUpMlDsa();
        account = _newAccount(address(registry), STAKE);
        _useStandIns(account);
    }

    function _expect(bytes4 selector) private {
        vm.expectRevert(selector);
    }

    // Deployment

    function test_the_constructor_binds_the_verifier_and_the_blob() public view {
        _assertTrue(address(account.verifier()) == address(verifier), "verifier");
        _assertEq(account.verifierCodeHash(), verifierHash, "verifier hash");
        _assertTrue(account.pkBlob() == pkBlob, "blob");
        _assertEq(account.pkBlobHash(), blobHash, "blob hash");
    }

    function test_the_constructor_stakes_with_a_registry() public view {
        _assertTrue(registry.staker() == address(account), "staker");
        _assertEq(registry.staked(), STAKE, "stake");
        _assertEq(uint256(registry.delay()), UNSTAKE_DELAY, "delay");
    }

    function test_a_zero_registry_means_no_stake() public {
        QDayMlDsaAccount unstaked = _newAccount(address(0), 0);
        _assertTrue(address(unstaked) != address(0), "deployed");
        _assertEq(address(unstaked).balance, 0, "balance");
    }

    function test_a_wrong_verifier_hash_is_refused() public {
        _expect(QDayMlDsaAccount.BadCodeHash.selector);
        new QDayMlDsaAccount(address(verifier), bytes32(uint256(1)), pkBlob, blobHash, address(0), 0, 0);
    }

    function test_a_wrong_blob_hash_is_refused() public {
        _expect(QDayMlDsaAccount.BadCodeHash.selector);
        new QDayMlDsaAccount(address(verifier), verifierHash, pkBlob, bytes32(uint256(1)), address(0), 0, 0);
    }

    function test_a_blob_of_the_wrong_size_is_refused() public {
        address short = _deployRaw(hex"00112233");
        _expect(QDayMlDsaAccount.BadBlob.selector);
        new QDayMlDsaAccount(address(verifier), verifierHash, short, short.codehash, address(0), 0, 0);
    }

    function test_helpers_are_deployed_with_the_exact_runtime_code() public {
        QDayMlDsaAccount fresh = _newAccount(address(0), 0);
        _assertEq(keccak256(fresh.approveOp().code), keccak256(hex"5f355f5faa"), "APPROVE");
        _assertEq(keccak256(fresh.txParamOp().code), keccak256(hex"5f35b05f5260205ff3"), "TXPARAM");
        _assertEq(keccak256(fresh.sigParamOp().code), keccak256(hex"6020355f35b45f5260205ff3"), "SIGPARAM");
        _assertEq(keccak256(fresh.sigDataCopyOp().code), keccak256(hex"6040356020355f355fb56020355ff3"), "SIGDATACOPY");
    }

    function test_constants_match_the_scheme() public view {
        _assertEq(account.SIG_BYTES(), 2420, "SIG_BYTES");
        _assertEq(account.BLOB_BYTES(), 20_545, "BLOB_BYTES");
    }

    function test_plain_eth_is_accepted() public {
        (bool ok,) = address(account).call{value: 1}("");
        _assertTrue(ok, "receive refused");
    }

    function test_unknown_calls_revert() public {
        (bool ok,) = address(account).call(hex"deadbeef");
        _assertFalse(ok, "there must be no fallback");
    }

    function test_there_is_no_key_index_or_rotation() public {
        (bool spend,) = address(account).call(abi.encodeWithSelector(0x92f59d73, uint256(0)));
        (bool rotate,) = address(account).call(abi.encodeWithSelector(0xed296e9c, hashA, hashB));
        _assertFalse(spend, "spendIndex must not exist");
        _assertFalse(rotate, "rotate must not exist");
    }

    // validate()

    function test_a_real_signature_is_approved() public {
        _transaction(account, hashA, _bytes("sigA"));
        account.validate();
        _assertApproved(account);
    }

    function test_the_same_key_can_sign_again() public {
        _transaction(account, hashA, _bytes("sigA"));
        account.validate();
        _transaction(account, hashB, _bytes("sigB"));
        account.validate();
        _assertApproved(account);
    }

    function test_another_transactions_hash_is_refused() public {
        _transaction(account, hashB, _bytes("sigA"));
        _expect(QDayMlDsaAccount.BadSignature.selector);
        account.validate();
    }

    function test_a_replay_at_the_next_nonce_is_refused() public {
        // The nonce is in the hash, so a replay arrives as a hash this signature doesn't cover.
        bytes memory sig = _bytes("sigA");
        _transaction(account, hashA, sig);
        account.validate();
        _transaction(account, keccak256(abi.encode(hashA, "next nonce")), sig);
        _expect(QDayMlDsaAccount.BadSignature.selector);
        account.validate();
    }

    function test_a_tampered_signature_is_refused() public {
        bytes memory sig = _bytes("sigA");
        sig[100] ^= 0x01;
        _transaction(account, hashA, sig);
        _expect(QDayMlDsaAccount.BadSignature.selector);
        account.validate();
    }

    function test_a_signature_from_another_key_is_refused() public {
        _transaction(account, hashA, _bytes("otherSigA"));
        _expect(QDayMlDsaAccount.BadSignature.selector);
        account.validate();
    }

    function test_a_wrong_length_signature_is_refused() public {
        _transaction(account, hashA, _bytes("sigA"));
        state.setSigLength(2419);
        _expect(QDayMlDsaAccount.BadSignature.selector);
        account.validate();
    }

    function test_a_protocol_checked_scheme_is_refused() public {
        state.setSignature(hashA, 1, _bytes("sigA"));
        _expect(QDayMlDsaAccount.BadSignature.selector);
        account.validate();
    }

    function test_a_swapped_blob_is_refused_at_use() public {
        vm.etch(pkBlob, _bytes("otherBlob"));
        _transaction(account, hashA, _bytes("otherSigA"));
        _expect(QDayMlDsaAccount.BadCodeHash.selector);
        account.validate();
    }

    function test_a_swapped_verifier_is_refused_at_use() public {
        // A verifier that says yes to everything.
        vm.etch(address(verifier), hex"60015f5260205ff3");
        _transaction(account, hashA, _bytes("sigA"));
        _expect(QDayMlDsaAccount.BadCodeHash.selector);
        account.validate();
    }

    function test_a_verifier_that_reverts_is_refused() public {
        // Swapping the verifier's own helper leaves its code hash alone, but makes it revert.
        vm.etch(verifierHelper, REVERTS);
        _transaction(account, hashA, _bytes("sigA"));
        _expect(QDayMlDsaAccount.BadSignature.selector);
        account.validate();
    }

    function test_a_failed_helper_call_reverts_opcode_failed() public {
        address[4] memory helpers =
            [account.txParamOp(), account.sigParamOp(), account.sigDataCopyOp(), account.approveOp()];
        for (uint256 i = 0; i < helpers.length; i++) {
            _useStandIns(account);
            _transaction(account, hashA, _bytes("sigA"));
            vm.etch(helpers[i], REVERTS);
            _expect(QDayMlDsaAccount.OpcodeFailed.selector);
            account.validate();
        }
    }

    function test_report_verify_gas() public {
        bytes[] memory samples = _list("gasSamples");
        uint256 total;
        uint256 least = type(uint256).max;
        uint256 most;
        for (uint256 i = 0; i < samples.length; i++) {
            bytes memory message = abi.encodePacked(keccak256(abi.encodePacked(uint32(i))));
            bytes memory sig = samples[i];
            uint256 before = gasleft();
            bool ok = verifier.verify(pkBlob, message, sig);
            uint256 used = before - gasleft();
            _assertTrue(ok, "sample refused");
            total += used;
            if (used < least) least = used;
            if (used > most) most = used;
        }
        emit log_named_uint("verify gas, least", least);
        emit log_named_uint("verify gas, average", total / samples.length);
        emit log_named_uint("verify gas, most", most);

        _transaction(account, hashA, _bytes("sigA"));
        uint256 start = gasleft();
        account.validate();
        emit log_named_uint("validate() gas with stand-in helpers", start - gasleft());
    }
}

contract QDayMlDsaFactoryTest is MlDsaBase {
    bytes32 private constant SALT = keccak256("salt");

    function setUp() public {
        _setUpMlDsa();
    }

    function _predict(address stakingRegistry, uint256 stake, bytes32 salt) private view returns (address) {
        return factory.predictAccount(
            address(verifier), verifierHash, pkBlob, blobHash, stakingRegistry, stake, UNSTAKE_DELAY, salt
        );
    }

    function _deploy(address stakingRegistry, uint256 stake, bytes32 salt) private returns (address) {
        return factory.deployAccount{value: stake}(
            address(verifier), verifierHash, pkBlob, blobHash, stakingRegistry, stake, UNSTAKE_DELAY, salt
        );
    }

    function test_a_blob_lands_where_predicted() public {
        address predicted = factory.predictBlob(blob);
        _assertEq(predicted.code.length, 0, "nothing there yet");
        address deployed = factory.deployBlob(blob);
        _assertTrue(deployed == predicted, "wrong address");
        _assertEq(keccak256(deployed.code), keccak256(blob), "the code is the blob, verbatim");
    }

    function test_deploying_a_blob_twice_gives_the_same_address() public {
        address first = factory.deployBlob(blob);
        _assertTrue(factory.deployBlob(blob) == first, "second deployment");
    }

    function test_a_factory_blob_binds_like_a_direct_one() public {
        address deployed = factory.deployBlob(blob);
        _assertEq(deployed.codehash, blobHash, "code hash");
    }

    function test_a_blob_of_the_wrong_size_is_refused() public {
        vm.expectRevert(QDayMlDsaFactory.BadBlob.selector);
        factory.deployBlob(hex"00112233");
    }

    function test_a_blob_without_its_prefix_is_refused() public {
        bytes memory changed = bytes.concat(blob);
        changed[0] = 0xEF;
        vm.expectRevert(QDayMlDsaFactory.BadBlob.selector);
        factory.deployBlob(changed);
    }

    function test_an_account_lands_where_predicted() public {
        address predicted = _predict(address(0), 0, SALT);
        _assertEq(predicted.code.length, 0, "nothing there yet");
        _assertTrue(_deploy(address(0), 0, SALT) == predicted, "wrong address");
        _assertTrue(predicted.code.length > 0, "deployed");
    }

    function test_each_salt_gives_its_own_address() public view {
        _assertTrue(_predict(address(0), 0, SALT) != _predict(address(0), 0, bytes32(0)), "salt");
    }

    function test_the_prediction_commits_to_the_key() public view {
        address other = factory.predictAccount(
            address(verifier), verifierHash, address(0x1234), blobHash, address(0), 0, UNSTAKE_DELAY, SALT
        );
        _assertTrue(other != _predict(address(0), 0, SALT), "the blob must change the address");
    }

    function test_the_factory_passes_the_stake_on() public {
        address deployed = _deploy(address(registry), STAKE, SALT);
        _assertTrue(registry.staker() == deployed, "staker");
        _assertEq(registry.staked(), STAKE, "stake");
    }

    function test_deploying_an_account_twice_fails() public {
        _deploy(address(0), 0, SALT);
        vm.expectRevert(QDayMlDsaFactory.DeployFailed.selector);
        _deploy(address(0), 0, SALT);
    }

    function test_predict_receive_deploy_spend() public {
        // Funds arrive before anything exists at the address.
        address predicted = _predict(address(0), 0, SALT);
        _assertEq(predicted.code.length, 0, "nothing there yet");
        (bool sent,) = predicted.call{value: 1 ether}("");
        _assertTrue(sent, "funding");
        _assertEq(predicted.balance, 1 ether, "funded");

        _assertTrue(_deploy(address(0), 0, SALT) == predicted, "deployed");
        _assertEq(predicted.balance, 1 ether, "the funds survive deployment");

        QDayMlDsaAccount account = QDayMlDsaAccount(payable(predicted));
        _useStandIns(account);
        _transaction(account, hashA, _bytes("sigA"));
        account.validate();
        _assertApproved(account);

        // After approval, the SENDER frame spends as the account.
        vm.prank(predicted);
        (bool spent,) = RECIPIENT.call{value: 0.25 ether}("");
        _assertTrue(spent, "spending");
        _assertEq(RECIPIENT.balance, 0.25 ether, "recipient");
        _assertEq(predicted.balance, 0.75 ether, "change");
    }

    receive() external payable {}
}
