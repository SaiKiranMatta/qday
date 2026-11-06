// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {QDayAccountFactory} from "../src/QDayAccountFactory.sol";
import {QDayFrameAccount} from "../src/QDayFrameAccount.sol";
import {XMSSVerifier} from "../src/XMSSVerifier.sol";
import {TestBase} from "./utils/TestBase.sol";

contract QDayAccountFactoryTest is TestBase {
    QDayAccountFactory private factory;
    address private verifier;

    bytes32 private constant ROOT = bytes32(uint256(0xa11ce));
    bytes32 private constant PUB_SEED = bytes32(uint256(0xb0b));
    bytes32 private constant NEXT_ROOT = bytes32(uint256(0xc0ffee));

    function setUp() public {
        verifier = address(new XMSSVerifier());
        factory = new QDayAccountFactory(verifier);
    }

    function test_the_address_is_known_before_deployment() public {
        address predicted = factory.accountAddress(ROOT, PUB_SEED);
        _assertEq(predicted.code.length, 0, "not deployed yet");
        address account = factory.createAccount(ROOT, PUB_SEED);
        _assertTrue(account == predicted, "deployed at the predicted address");
        _assertTrue(account.code.length > 0, "has code");
    }

    function test_funds_sent_before_deployment_stay_with_the_account() public {
        address predicted = factory.accountAddress(ROOT, PUB_SEED);
        vm.deal(predicted, 1 ether);
        factory.createAccount(ROOT, PUB_SEED);
        _assertEq(predicted.balance, 1 ether, "balance");
    }

    function test_the_account_gets_the_key_and_the_shared_verifier() public {
        QDayFrameAccount account = QDayFrameAccount(payable(factory.createAccount(ROOT, PUB_SEED)));
        _assertEq(account.root(), ROOT, "root");
        _assertEq(account.pubSeed(), PUB_SEED, "pubSeed");
        _assertTrue(address(account.verifier()) == verifier, "verifier");
        _assertTrue(account.factory() == address(factory), "factory");
        _assertTrue(factory.accountOf(ROOT, PUB_SEED) == address(account), "found by root");
    }

    function test_creating_twice_fails() public {
        factory.createAccount(ROOT, PUB_SEED);
        vm.expectRevert(bytes("create failed"));
        factory.createAccount(ROOT, PUB_SEED);
    }

    function test_different_keys_get_different_addresses() public view {
        _assertTrue(
            factory.accountAddress(ROOT, PUB_SEED) != factory.accountAddress(NEXT_ROOT, PUB_SEED), "different addresses"
        );
    }

    function test_a_rotated_account_is_found_by_its_new_root() public {
        QDayFrameAccount account = QDayFrameAccount(payable(factory.createAccount(ROOT, PUB_SEED)));
        vm.prank(address(account));
        account.rotate(NEXT_ROOT, PUB_SEED);
        _assertEq(account.root(), NEXT_ROOT, "rotated root");
        _assertTrue(factory.rotatedAccountOf(NEXT_ROOT) == address(account), "found by new root");
        _assertTrue(factory.accountOf(NEXT_ROOT, PUB_SEED) == address(account), "found via accountOf");
        _assertTrue(factory.accountOf(ROOT, PUB_SEED) == address(account), "still found by first root");
    }

    function test_only_accounts_can_record_roots() public {
        vm.expectRevert(bytes("not a QDay account"));
        factory.recordRoot(ROOT, PUB_SEED, NEXT_ROOT);
    }
}
