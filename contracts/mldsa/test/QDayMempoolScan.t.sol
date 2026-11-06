// SPDX-License-Identifier: MIT
pragma solidity ^0.8.25;

import {Test} from "forge-std/Test.sol";

import {MLDSA44Verifier} from "../src/MLDSA44Verifier.sol";
import {deployF1600_170} from "./ZZZ_FastKeccak170.sol";

/// The mempool refuses any validation that runs a banned opcode. It decides by tracing, so this
/// scan is stricter: it refuses a banned opcode anywhere in the code, run or not.
contract QDayMempoolScanTest is Test {
    uint8 private constant GAS = 0x5a;

    function _isCall(uint8 op) private pure returns (bool) {
        return op == 0xf1 || op == 0xf2 || op == 0xf4 || op == 0xfa;
    }

    /// The opcodes banned in validation, apart from GAS, which is allowed directly before a call.
    function _isBanned(uint8 op) private pure returns (bool) {
        return op == 0x3a // GASPRICE
            || op == 0x45 // GASLIMIT
            || op == 0x44 // DIFFICULTY, PREVRANDAO
            || op == 0x42 // TIMESTAMP
            || op == 0x48 // BASEFEE
            || op == 0x40 // BLOCKHASH
            || op == 0x43 // NUMBER
            || op == 0x47 // SELFBALANCE
            || op == 0x31 // BALANCE
            || op == 0x32 // ORIGIN
            || op == 0x41 // COINBASE
            || op == 0xff // SELFDESTRUCT
            || op == 0xfe // INVALID
            || op == 0x49 // BLOBHASH
            || op == 0x4a; // BLOBBASEFEE
    }

    /// Where the code ends. The compiler appends a CBOR metadata map and its length, and puts one
    /// INVALID byte in front of them as a terminator. That byte is only left out here if the
    /// instruction before it stops execution, because then nothing can reach it.
    function _codeEnd(bytes memory code) private pure returns (uint256) {
        if (code.length < 2) return code.length;
        uint256 size = (uint256(uint8(code[code.length - 2])) << 8) | uint8(code[code.length - 1]);
        if (size + 3 > code.length) return code.length;
        uint256 metadataAt = code.length - 2 - size;
        uint8 first = uint8(code[metadataAt]);
        if (first != 0xa1 && first != 0xa2 && first != 0xa3) return code.length;
        if (metadataAt >= 2 && uint8(code[metadataAt - 1]) == 0xfe) {
            uint8 before = uint8(code[metadataAt - 2]);
            bool stops = before == 0x00 || before == 0x56 || before == 0xf3 || before == 0xfd;
            if (stops) return metadataAt - 1;
        }
        return metadataAt;
    }

    /// Walks the opcodes, skipping PUSH data, and returns the first offence and how many there are.
    function _scan(bytes memory code)
        private
        pure
        returns (uint256 offences, uint256 firstAt, uint8 firstOp, uint256 callGas)
    {
        uint256 end = _codeEnd(code);
        for (uint256 i = 0; i < end; i++) {
            uint8 op = uint8(code[i]);
            bool bad = _isBanned(op);
            if (op == GAS) {
                if (i + 1 < end && _isCall(uint8(code[i + 1]))) {
                    callGas += 1;
                } else {
                    bad = true;
                }
            }
            if (bad) {
                if (offences == 0) {
                    firstAt = i;
                    firstOp = op;
                }
                offences += 1;
            }
            if (op >= 0x60 && op <= 0x7f) i += op - 0x5f;
        }
    }

    function _assertClean(bytes memory code, string memory name) private {
        (uint256 offences, uint256 firstAt, uint8 firstOp, uint256 callGas) = _scan(code);
        emit log_named_uint(string.concat(name, " runtime bytes"), code.length);
        emit log_named_uint(string.concat(name, " GAS before a call"), callGas);
        emit log_named_uint(string.concat(name, " offences"), offences);
        if (offences > 0) {
            emit log_named_uint("first offence at", firstAt);
            emit log_named_uint("first offending opcode", firstOp);
        }
        assertEq(offences, 0, "banned opcode in the code");
    }

    function test_verifier_has_no_banned_opcodes() public {
        MLDSA44Verifier verifier = new MLDSA44Verifier(deployF1600_170());
        _assertClean(address(verifier).code, "verifier");
    }

    function test_verifier_is_the_expected_size() public {
        MLDSA44Verifier verifier = new MLDSA44Verifier(deployF1600_170());
        assertEq(address(verifier).code.length, 24_163);
    }

    function test_helper_has_no_banned_opcodes() public {
        _assertClean(deployF1600_170().code, "helper");
    }

    function test_the_scan_catches_a_standalone_gas() public pure {
        (uint256 offences, uint256 at, uint8 op,) = _scan(hex"5b5a50");
        assertEq(offences, 1);
        assertEq(at, 1);
        assertEq(op, GAS);
    }

    function test_the_scan_allows_gas_directly_before_a_call() public pure {
        (uint256 offences,,, uint256 callGas) = _scan(hex"5afa505af1505af4505af2");
        assertEq(offences, 0);
        assertEq(callGas, 4);
    }

    function test_the_scan_refuses_gas_two_bytes_before_a_call() public pure {
        (uint256 offences,,,) = _scan(hex"5a50fa");
        assertEq(offences, 1);
    }

    function test_the_scan_skips_push_data() public pure {
        (uint256 offences,,,) = _scan(hex"615a5a50");
        assertEq(offences, 0);
    }

    function test_the_scan_leaves_out_the_terminator_after_a_jump() public pure {
        // JUMP, INVALID, then a 3-byte map and its length.
        (uint256 offences,,,) = _scan(hex"56fea16161" hex"0003");
        assertEq(offences, 0);
    }

    function test_the_scan_refuses_a_terminator_that_can_be_reached() public pure {
        // JUMPDEST runs straight into the INVALID.
        (uint256 offences,, uint8 op,) = _scan(hex"5bfea16161" hex"0003");
        assertEq(offences, 1);
        assertEq(op, 0xfe);
    }

    function test_the_scan_refuses_invalid_in_the_middle_of_code() public pure {
        (uint256 offences,,,) = _scan(hex"56fe5b56fea16161" hex"0003");
        assertEq(offences, 1);
    }

    function test_the_scan_catches_each_banned_opcode() public pure {
        bytes memory banned = hex"3a45444248404347313241fffe494a";
        for (uint256 i = 0; i < banned.length; i++) {
            bytes memory one = new bytes(1);
            one[0] = banned[i];
            (uint256 offences,,,) = _scan(one);
            assertEq(offences, 1);
        }
    }
}
