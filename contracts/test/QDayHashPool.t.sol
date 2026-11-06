// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {QDayHashPool} from "../src/QDayHashPool.sol";
import {TestBase} from "./utils/TestBase.sol";

/// @notice A payee that refuses payments.
contract RefusesPayment {
    receive() external payable {
        revert("no");
    }
}

/// @notice A payee that burns all the gas it's given.
contract BurnsGas {
    receive() external payable {
        while (true) {}
    }
}

/// @notice A submitter that tries to spend again from inside its fee payment.
contract Reenters {
    QDayHashPool private immutable pool;
    bytes32 private leaf;
    bytes32 private seed;
    bytes private signature;
    address private to;
    uint256 private fee;
    bool public blocked;

    constructor(QDayHashPool pool_) {
        pool = pool_;
    }

    function run(bytes32 leaf_, bytes32 seed_, bytes calldata signature_, address to_, uint256 fee_) external {
        (leaf, seed, signature, to, fee) = (leaf_, seed_, signature_, to_, fee_);
        pool.spend(leaf, seed, signature, to, fee);
    }

    receive() external payable {
        try pool.spend(leaf, seed, signature, to, fee) {}
        catch {
            blocked = true;
        }
    }
}

contract QDayHashPoolTest is TestBase {
    event log_named_uint(string key, uint256 val);

    /// Matches the fixtures.
    address private constant POOL = 0x9090909090909090909090909090909090909090;
    address private constant TO = 0xBEbeBeBEbeBebeBeBEBEbebEBeBeBebeBeBebebe;
    uint256 private constant FEE = 0.01 ether;
    uint256 private constant CHAIN_ID = 1337;

    QDayHashPool private pool = QDayHashPool(POOL);

    function setUp() public {
        _loadFixtures("test/fixtures/hashpool.json");
        vm.etch(POOL, type(QDayHashPool).runtimeCode);
        vm.chainId(CHAIN_ID);
        vm.etch(TO, "");
        vm.deal(TO, 0);
        vm.deal(address(this), 100 ether);
    }

    receive() external payable {}

    function test_a_payment_is_held_under_its_leaf() public {
        pool.pay{value: 1 ether}(_word("leaf0"));
        _assertEq(pool.notes(_word("leaf0")), 1 ether, "not held");
        _assertEq(POOL.balance, 1 ether, "not received");
    }

    function test_paying_twice_adds_up() public {
        pool.pay{value: 1 ether}(_word("leaf0"));
        pool.pay{value: 2 ether}(_word("leaf0"));
        _assertEq(pool.notes(_word("leaf0")), 3 ether, "not added");
    }

    function test_paying_nothing_is_refused() public {
        bytes32 leaf = _word("leaf0");
        vm.expectRevert(QDayHashPool.NothingPaid.selector);
        pool.pay(leaf);
    }

    function test_the_spend_hash_binds_the_chain() public {
        bytes32 here = pool.spendHash(_word("leaf0"), TO, FEE);
        vm.chainId(1);
        _assertTrue(here != pool.spendHash(_word("leaf0"), TO, FEE), "chain not bound");
    }

    function test_a_spend_pays_the_recipient_and_the_submitter() public {
        pool.pay{value: 1 ether}(_word("leaf0"));
        address relayer = address(0xFEE);
        vm.prank(relayer);
        pool.spend(_word("leaf0"), _word("seed0"), _bytes("sig0"), TO, FEE);
        _assertEq(TO.balance, 1 ether - FEE, "recipient");
        _assertEq(relayer.balance, FEE, "submitter");
        _assertEq(pool.notes(_word("leaf0")), pool.SPENT(), "not marked");
    }

    function test_a_top_up_cant_block_a_spend() public {
        pool.pay{value: 1 ether}(_word("leaf0"));
        pool.pay{value: 1}(_word("leaf0"));
        pool.spend(_word("leaf0"), _word("seed0"), _bytes("sig0"), TO, FEE);
        _assertEq(TO.balance, 1 ether + 1 - FEE, "the whole note goes");
    }

    function test_a_key_is_spent_once() public {
        pool.pay{value: 1 ether}(_word("leaf0"));
        bytes32 leaf = _word("leaf0");
        bytes32 seed = _word("seed0");
        bytes memory signature = _bytes("sig0");
        pool.spend(leaf, seed, signature, TO, FEE);
        vm.expectRevert(QDayHashPool.AlreadySpent.selector);
        pool.spend(leaf, seed, signature, TO, FEE);
    }

    function test_a_spent_key_cant_be_paid_again() public {
        pool.pay{value: 1 ether}(_word("leaf0"));
        bytes32 leaf = _word("leaf0");
        pool.spend(leaf, _word("seed0"), _bytes("sig0"), TO, FEE);
        vm.expectRevert(QDayHashPool.AlreadySpent.selector);
        pool.pay{value: 1 ether}(leaf);
    }

    function test_an_empty_note_cant_be_spent() public {
        bytes32 leaf = _word("leaf0");
        bytes32 seed = _word("seed0");
        bytes memory signature = _bytes("sig0");
        vm.expectRevert(QDayHashPool.NoNote.selector);
        pool.spend(leaf, seed, signature, TO, FEE);
    }

    function _expectRefused(bytes32 leaf, bytes32 seed, bytes memory signature, address to, uint256 fee) private {
        vm.expectRevert(QDayHashPool.BadSignature.selector);
        pool.spend(leaf, seed, signature, to, fee);
    }

    function test_a_changed_recipient_is_refused() public {
        pool.pay{value: 1 ether}(_word("leaf0"));
        _expectRefused(_word("leaf0"), _word("seed0"), _bytes("sig0"), address(0xBAD), FEE);
    }

    function test_a_changed_fee_is_refused() public {
        pool.pay{value: 1 ether}(_word("leaf0"));
        _expectRefused(_word("leaf0"), _word("seed0"), _bytes("sig0"), TO, FEE + 1);
    }

    function test_another_chain_is_refused() public {
        pool.pay{value: 1 ether}(_word("leaf0"));
        vm.chainId(1);
        _expectRefused(_word("leaf0"), _word("seed0"), _bytes("sig0"), TO, FEE);
    }

    function test_another_keys_signature_is_refused() public {
        pool.pay{value: 1 ether}(_word("leaf0"));
        _expectRefused(_word("leaf0"), _word("seed1"), _bytes("sig1"), TO, FEE);
    }

    function test_a_wrong_public_seed_is_refused() public {
        pool.pay{value: 1 ether}(_word("leaf0"));
        _expectRefused(_word("leaf0"), _word("seed1"), _bytes("sig0"), TO, FEE);
    }

    function test_a_changed_signature_is_refused() public {
        pool.pay{value: 1 ether}(_word("leaf0"));
        bytes memory signature = _bytes("sig0");
        for (uint256 at = 0; at < signature.length; at += 311) {
            signature[at] ^= 0x01;
            _expectRefused(_word("leaf0"), _word("seed0"), signature, TO, FEE);
            signature[at] ^= 0x01;
        }
    }

    function test_a_wrong_length_is_refused() public {
        pool.pay{value: 1 ether}(_word("leaf0"));
        bytes memory good = _bytes("sig0");
        bytes memory short = new bytes(good.length - 1);
        bytes memory long = new bytes(good.length + 1);
        for (uint256 i = 0; i < short.length; i++) {
            short[i] = good[i];
            long[i] = good[i];
        }
        _expectRefused(_word("leaf0"), _word("seed0"), short, TO, FEE);
        _expectRefused(_word("leaf0"), _word("seed0"), long, TO, FEE);
    }

    function test_a_fee_above_the_note_takes_what_there_is() public {
        pool.pay{value: FEE / 2}(_word("leaf0"));
        address relayer = address(0xFEE);
        vm.prank(relayer);
        pool.spend(_word("leaf0"), _word("seed0"), _bytes("sig0"), TO, FEE);
        _assertEq(relayer.balance, FEE / 2, "capped fee");
        _assertEq(TO.balance, 0, "nothing left");
    }

    function test_a_payee_that_refuses_is_kept_for_withdrawal() public {
        pool.pay{value: 1 ether}(_word("leaf0"));
        vm.etch(TO, address(new RefusesPayment()).code);
        pool.spend(_word("leaf0"), _word("seed0"), _bytes("sig0"), TO, FEE);
        _assertEq(pool.notes(_word("leaf0")), pool.SPENT(), "still spent");
        _assertEq(pool.owed(TO), 1 ether - FEE, "not kept");
        vm.etch(TO, "");
        vm.prank(TO);
        pool.withdraw();
        _assertEq(TO.balance, 1 ether - FEE, "not withdrawn");
        _assertEq(pool.owed(TO), 0, "not cleared");
    }

    function test_a_payee_that_burns_gas_doesnt_stop_the_spend() public {
        pool.pay{value: 1 ether}(_word("leaf0"));
        vm.etch(TO, address(new BurnsGas()).code);
        pool.spend(_word("leaf0"), _word("seed0"), _bytes("sig0"), TO, FEE);
        _assertEq(pool.owed(TO), 1 ether - FEE, "not kept");
    }

    function test_a_submitter_that_refuses_is_kept_for_withdrawal() public {
        pool.pay{value: 1 ether}(_word("leaf0"));
        RefusesPayment submitter = new RefusesPayment();
        vm.prank(address(submitter));
        pool.spend(_word("leaf0"), _word("seed0"), _bytes("sig0"), TO, FEE);
        _assertEq(pool.owed(address(submitter)), FEE, "fee not kept");
        _assertEq(TO.balance, 1 ether - FEE, "recipient still paid");
    }

    function test_withdrawing_nothing_is_refused() public {
        vm.expectRevert(QDayHashPool.NothingOwed.selector);
        pool.withdraw();
    }

    function test_a_failed_withdrawal_keeps_what_is_owed() public {
        pool.pay{value: 1 ether}(_word("leaf0"));
        vm.etch(TO, address(new RefusesPayment()).code);
        pool.spend(_word("leaf0"), _word("seed0"), _bytes("sig0"), TO, FEE);
        vm.prank(TO);
        vm.expectRevert(QDayHashPool.WithdrawFailed.selector);
        pool.withdraw();
        _assertEq(pool.owed(TO), 1 ether - FEE, "lost");
    }

    function test_reentering_from_the_fee_payment_gets_nothing_more() public {
        pool.pay{value: 1 ether}(_word("leaf0"));
        pool.pay{value: 1 ether}(_word("leaf1"));
        Reenters submitter = new Reenters(pool);
        submitter.run(_word("leaf0"), _word("seed0"), _bytes("sig0"), TO, FEE);
        _assertTrue(submitter.blocked(), "the second spend wasn't refused");
        _assertEq(TO.balance, 1 ether - FEE, "paid twice");
    }

    function test_notes_are_independent() public {
        pool.pay{value: 1 ether}(_word("leaf0"));
        pool.pay{value: 2 ether}(_word("leaf1"));
        pool.spend(_word("leaf0"), _word("seed0"), _bytes("sig0"), TO, FEE);
        _assertEq(pool.notes(_word("leaf1")), 2 ether, "the other note moved");
        pool.spend(_word("leaf1"), _word("seed1"), _bytes("sig1"), TO, FEE);
        _assertEq(TO.balance, 3 ether - 2 * FEE, "total");
    }

    function test_spend_gas() public {
        for (uint256 i = 0; i < 4; i++) {
            string memory n = i == 0 ? "0" : i == 1 ? "1" : i == 2 ? "2" : "3";
            bytes32 leaf = _word(string.concat("leaf", n));
            bytes32 seed = _word(string.concat("seed", n));
            bytes memory signature = _bytes(string.concat("sig", n));
            pool.pay{value: 1 ether}(leaf);
            uint256 before = gasleft();
            pool.spend(leaf, seed, signature, TO, FEE);
            emit log_named_uint("spend gas", before - gasleft());
        }
    }
}
