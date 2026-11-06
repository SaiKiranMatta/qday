// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {XMSSVerifier} from "../src/XMSSVerifier.sol";
import {TestBase} from "./utils/TestBase.sol";

contract XMSSVerifierTest is TestBase {
    event log_named_uint(string key, uint256 val);

    XMSSVerifier private verifier;
    bytes32 private root;
    bytes32 private pubSeed;
    bytes32 private constant VECTOR_MESSAGE =
        0x1111111111111111111111111111111111111111111111111111111111111111;

    function setUp() public {
        _loadFixtures();
        verifier = new XMSSVerifier();
        root = _word("root0");
        pubSeed = _word("pubSeed0");
    }

    function test_the_published_signature_verifies() public view {
        bytes memory sig = _bytes("vector");
        _assertEq(
            keccak256(sig),
            0x8937def506577209cdaaff87612f54af2e417d5502132267f358b5daa32ece1f,
            "fixture is not the published vector"
        );
        _assertTrue(verifier.verify(root, pubSeed, VECTOR_MESSAGE, sig), "vector refused");
    }

    function test_first_and_last_reachable_keys_verify() public view {
        _assertTrue(verifier.verify(root, pubSeed, _word("hashA"), _bytes("sig5")), "key 5");
        _assertTrue(verifier.verify(root, pubSeed, _word("hashA"), _bytes("sig1007")), "key 1007");
    }

    function test_every_sample_verifies() public view {
        bytes[] memory samples = _list("gasSamples");
        for (uint256 i = 0; i < samples.length; i++) {
            bytes32 message = keccak256(abi.encodePacked(uint32(i)));
            _assertTrue(verifier.verify(root, pubSeed, message, samples[i]), "sample refused");
        }
    }

    function test_another_message_is_false() public view {
        _assertFalse(verifier.verify(root, pubSeed, _word("hashB"), _bytes("sig5")), "accepted");
    }

    function test_another_root_is_false() public view {
        _assertFalse(
            verifier.verify(_word("root1"), pubSeed, _word("hashA"), _bytes("sig5")), "accepted"
        );
    }

    function test_another_pub_seed_is_false() public view {
        _assertFalse(
            verifier.verify(root, _word("pubSeed1"), _word("hashA"), _bytes("sig5")), "accepted"
        );
    }

    function test_changing_any_byte_is_false() public view {
        bytes memory sig = _bytes("sig5");
        bytes32 message = _word("hashA");
        for (uint256 i = 0; i < sig.length; i++) {
            sig[i] ^= 0x01;
            _assertFalse(verifier.verify(root, pubSeed, message, sig), "tampered accepted");
            sig[i] ^= 0x01;
        }
        _assertTrue(verifier.verify(root, pubSeed, message, sig), "restored refused");
    }

    function test_wrong_lengths_are_false() public view {
        bytes memory sig = _bytes("sig5");
        bytes32 message = _word("hashA");
        _assertFalse(verifier.verify(root, pubSeed, message, ""), "empty accepted");

        bytes memory shorter = new bytes(2499);
        bytes memory longer = new bytes(2501);
        for (uint256 i = 0; i < 2499; i++) {
            shorter[i] = sig[i];
            longer[i] = sig[i];
        }
        longer[2499] = sig[2499];
        _assertFalse(verifier.verify(root, pubSeed, message, shorter), "short accepted");
        _assertFalse(verifier.verify(root, pubSeed, message, longer), "long accepted");
    }

    function test_an_index_past_the_tree_is_false() public view {
        bytes memory sig = _bytes("sig5");
        bytes32 message = _word("hashA");
        sig[2] = 0x04; // index 1024 + 5
        _assertFalse(verifier.verify(root, pubSeed, message, sig), "1029 accepted");
        sig[0] = 0xFF;
        _assertFalse(verifier.verify(root, pubSeed, message, sig), "huge index accepted");
    }

    function test_report_average_gas() public {
        bytes[] memory samples = _list("gasSamples");
        uint256 total;
        uint256 least = type(uint256).max;
        uint256 most;
        for (uint256 i = 0; i < samples.length; i++) {
            bytes32 message = keccak256(abi.encodePacked(uint32(i)));
            bytes memory sig = samples[i];
            uint256 before = gasleft();
            bool ok = verifier.verify(root, pubSeed, message, sig);
            uint256 used = before - gasleft();
            _assertTrue(ok, "sample refused");
            total += used;
            if (used < least) least = used;
            if (used > most) most = used;
        }
        emit log_named_uint("verify gas, least", least);
        emit log_named_uint("verify gas, average", total / samples.length);
        emit log_named_uint("verify gas, most", most);
    }
}
