// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {IMLDSAInlineVerifier} from "../src/IMLDSAInlineVerifier.sol";
import {QDayLatticePool} from "../src/QDayLatticePool.sol";
import {TestBase} from "./utils/TestBase.sol";

/// @notice A payee that refuses payments.
contract Refuses {
    receive() external payable {
        revert("no");
    }
}

/// @notice A payee that burns all the gas it's given.
contract Burns {
    receive() external payable {
        while (true) {}
    }
}

/// Real keys and signatures from the Rust signer, through the real verifier.
contract QDayLatticePoolTest is TestBase {
    event log_named_uint(string key, uint256 val);

    /// Matches the fixtures.
    address private constant POOL = 0x9090909090909090909090909090909090909090;
    address private constant TO = 0xBEbeBeBEbeBebeBeBEBEbebEBeBeBebeBeBebebe;
    uint256 private constant FEE = 0.01 ether;
    bytes private constant CIPHERTEXT = hex"11";

    QDayLatticePool private pool = QDayLatticePool(POOL);
    IMLDSAInlineVerifier private verifier;
    address private matrix;
    bytes private ciphertext;

    function _deployRaw(bytes memory runtime) private returns (address at) {
        bytes memory code = abi.encodePacked(
            bytes1(0x61), uint16(runtime.length), hex"600e600039", bytes1(0x61), uint16(runtime.length), hex"6000f3", runtime
        );
        assembly {
            at := create(0, add(code, 32), mload(code))
        }
        require(at != address(0), "raw deploy failed");
    }

    function setUp() public {
        _loadFixtures("test/fixtures/lattice.json");
        address helper = _deployRaw(_bytes("helperRuntime"));
        bytes memory code = abi.encodePacked(_bytes("verifierInitcode"), abi.encode(helper));
        address at;
        assembly {
            at := create(0, add(code, 32), mload(code))
        }
        require(at != address(0), "verifier deploy failed");
        verifier = IMLDSAInlineVerifier(at);
        matrix = _deployRaw(_bytes("matrix"));
        vm.etch(POOL, address(new QDayLatticePool(verifier, matrix)).code);
        vm.chainId(1337);
        vm.etch(TO, "");
        vm.deal(TO, 0);
        vm.deal(address(this), 100 ether);
        ciphertext = new bytes(1120);
    }

    receive() external payable {}

    function _hash(string memory name) private view returns (bytes32) {
        return keccak256(_bytes(name));
    }

    function _pay(string memory name, uint256 value) private {
        pool.pay{value: value}(_hash(name), 0x00, ciphertext);
    }

    function test_a_payment_is_held_under_its_key() public {
        _pay("key0", 1 ether);
        _assertEq(pool.notes(_hash("key0")), 1 ether, "not held");
    }

    function test_paying_nothing_is_refused() public {
        bytes32 keyHash = _hash("key0");
        vm.expectRevert(QDayLatticePool.NothingPaid.selector);
        pool.pay(keyHash, 0x00, ciphertext);
    }

    function test_a_short_ciphertext_is_refused() public {
        bytes32 keyHash = _hash("key0");
        vm.expectRevert(QDayLatticePool.BadCiphertext.selector);
        pool.pay{value: 1}(keyHash, 0x00, CIPHERTEXT);
    }

    function test_a_spend_pays_the_recipient_and_the_submitter() public {
        _pay("key0", 1 ether);
        address relayer = address(0xFEE);
        vm.prank(relayer);
        pool.spend(_bytes("key0"), _bytes("sig0"), TO, FEE);
        _assertEq(TO.balance, 1 ether - FEE, "recipient");
        _assertEq(relayer.balance, FEE, "submitter");
        _assertEq(pool.notes(_hash("key0")), pool.SPENT(), "not marked");
    }

    function test_a_top_up_cant_block_a_spend() public {
        _pay("key0", 1 ether);
        _pay("key0", 1);
        pool.spend(_bytes("key0"), _bytes("sig0"), TO, FEE);
        _assertEq(TO.balance, 1 ether + 1 - FEE, "the whole note goes");
    }

    function test_a_key_is_spent_once() public {
        _pay("key0", 1 ether);
        bytes memory key = _bytes("key0");
        bytes memory signature = _bytes("sig0");
        pool.spend(key, signature, TO, FEE);
        vm.expectRevert(QDayLatticePool.AlreadySpent.selector);
        pool.spend(key, signature, TO, FEE);
    }

    function test_a_spent_key_cant_be_paid_again() public {
        _pay("key0", 1 ether);
        pool.spend(_bytes("key0"), _bytes("sig0"), TO, FEE);
        bytes32 keyHash = _hash("key0");
        vm.expectRevert(QDayLatticePool.AlreadySpent.selector);
        pool.pay{value: 1}(keyHash, 0x00, ciphertext);
    }

    function test_an_empty_note_cant_be_spent() public {
        bytes memory key = _bytes("key0");
        bytes memory signature = _bytes("sig0");
        vm.expectRevert(QDayLatticePool.NoNote.selector);
        pool.spend(key, signature, TO, FEE);
    }

    function _refused(bytes memory key, bytes memory signature, address to, uint256 fee) private {
        vm.expectRevert(QDayLatticePool.BadSignature.selector);
        pool.spend(key, signature, to, fee);
    }

    function test_a_changed_recipient_is_refused() public {
        _pay("key0", 1 ether);
        _refused(_bytes("key0"), _bytes("sig0"), address(0xBAD), FEE);
    }

    function test_a_changed_fee_is_refused() public {
        _pay("key0", 1 ether);
        _refused(_bytes("key0"), _bytes("sig0"), TO, FEE + 1);
    }

    function test_another_chain_is_refused() public {
        _pay("key0", 1 ether);
        vm.chainId(1);
        _refused(_bytes("key0"), _bytes("sig0"), TO, FEE);
    }

    function test_another_pool_is_refused() public {
        QDayLatticePool other = new QDayLatticePool(verifier, matrix);
        bytes32 keyHash = _hash("key0");
        other.pay{value: 1 ether}(keyHash, 0x00, ciphertext);
        bytes memory key = _bytes("key0");
        bytes memory signature = _bytes("sig0");
        vm.expectRevert(QDayLatticePool.BadSignature.selector);
        other.spend(key, signature, TO, FEE);
    }

    function test_another_keys_signature_is_refused() public {
        _pay("key0", 1 ether);
        _refused(_bytes("key0"), _bytes("sig1"), TO, FEE);
    }

    function test_someone_elses_master_is_refused() public {
        _pay("key0", 1 ether);
        _refused(_bytes("key0"), _bytes("strangerSig"), TO, FEE);
    }

    function test_a_changed_signature_is_refused() public {
        _pay("key0", 1 ether);
        bytes memory signature = _bytes("sig0");
        signature[0] ^= 0x01;
        _refused(_bytes("key0"), signature, TO, FEE);
    }

    function test_a_short_signature_is_refused() public {
        _pay("key0", 1 ether);
        _refused(_bytes("key0"), new bytes(2419), TO, FEE);
    }

    function test_a_key_of_the_wrong_length_is_refused() public {
        bytes memory longer = abi.encodePacked(_bytes("key0"), bytes1(0));
        pool.pay{value: 1 ether}(keccak256(longer), 0x00, ciphertext);
        bytes memory signature = _bytes("sig0");
        vm.expectRevert(QDayLatticePool.BadKey.selector);
        pool.spend(longer, signature, TO, FEE);
    }

    function test_a_changed_matrix_is_refused() public {
        _pay("key0", 1 ether);
        bytes memory signature = _bytes("sig0");
        bytes memory key = _bytes("key0");
        vm.etch(matrix, new bytes(16385));
        vm.expectRevert(QDayLatticePool.BadCodeHash.selector);
        pool.spend(key, signature, TO, FEE);
    }

    function test_a_fee_above_the_note_takes_what_there_is() public {
        _pay("key0", FEE / 2);
        address relayer = address(0xFEE);
        vm.prank(relayer);
        pool.spend(_bytes("key0"), _bytes("sig0"), TO, FEE);
        _assertEq(relayer.balance, FEE / 2, "capped fee");
    }

    function test_a_payee_that_refuses_is_kept_for_withdrawal() public {
        _pay("key0", 1 ether);
        vm.etch(TO, address(new Refuses()).code);
        pool.spend(_bytes("key0"), _bytes("sig0"), TO, FEE);
        _assertEq(pool.owed(TO), 1 ether - FEE, "not kept");
        vm.etch(TO, "");
        vm.prank(TO);
        pool.withdraw();
        _assertEq(TO.balance, 1 ether - FEE, "not withdrawn");
        _assertEq(pool.owed(TO), 0, "not cleared");
    }

    function test_a_payee_that_burns_gas_costs_the_submitter_little() public {
        _pay("key0", 1 ether);
        vm.etch(TO, address(new Burns()).code);
        bytes memory key = _bytes("key0");
        bytes memory signature = _bytes("sig0");
        uint256 before = gasleft();
        pool.spend(key, signature, TO, FEE);
        _assertTrue(before - gasleft() < 1_700_000, "the payee burned the submitter's gas");
        _assertEq(pool.owed(TO), 1 ether - FEE, "not kept");
    }

    function test_withdrawing_nothing_is_refused() public {
        vm.expectRevert(QDayLatticePool.NothingOwed.selector);
        pool.withdraw();
    }

    function test_a_failed_withdrawal_keeps_what_is_owed() public {
        _pay("key0", 1 ether);
        vm.etch(TO, address(new Refuses()).code);
        pool.spend(_bytes("key0"), _bytes("sig0"), TO, FEE);
        vm.prank(TO);
        vm.expectRevert(QDayLatticePool.WithdrawFailed.selector);
        pool.withdraw();
        _assertEq(pool.owed(TO), 1 ether - FEE, "lost");
    }

    function test_notes_are_independent() public {
        _pay("key0", 1 ether);
        _pay("key1", 2 ether);
        pool.spend(_bytes("key0"), _bytes("sig0"), TO, FEE);
        _assertEq(pool.notes(_hash("key1")), 2 ether, "the other note moved");
        pool.spend(_bytes("key1"), _bytes("sig1"), TO, FEE);
        _assertEq(TO.balance, 3 ether - 2 * FEE, "total");
    }

    function test_gas() public {
        for (uint256 i = 0; i < 4; i++) {
            string memory n = i == 0 ? "0" : i == 1 ? "1" : i == 2 ? "2" : "3";
            bytes memory key = _bytes(string.concat("key", n));
            bytes memory signature = _bytes(string.concat("sig", n));
            bytes32 keyHash = keccak256(key);
            uint256 before = gasleft();
            pool.pay{value: 1 ether}(keyHash, 0x00, ciphertext);
            emit log_named_uint("pay gas", before - gasleft());
            before = gasleft();
            pool.spend(key, signature, TO, FEE);
            emit log_named_uint("spend gas", before - gasleft());
        }
    }
}
