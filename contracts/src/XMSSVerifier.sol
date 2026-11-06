// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

/// @notice Checks QDay signatures, as defined in docs/xmss-spec.md.
/// @dev The whole check is one assembly block that ends in `return`, so it owns all of memory.
///      Chain and node hashes use bytes 0x00-0x7f. The leaf hash input starts at 0x80.
contract XMSSVerifier {
    /// @notice Returns true if `sig` is a valid signature of `message`. Malformed input is false.
    function verify(bytes32 root, bytes32 pubSeed, bytes32 message, bytes calldata sig) external pure returns (bool) {
        assembly {
            /// Finishes chain `chain` from step `digit` and stores its end for the leaf hash.
            /// `leafBits` is the one-time key's place in the address.
            function finishChain(chain, digit, leafBits, chainsAt) {
                let position := shl(5, chain)
                mstore(0x40, calldataload(add(chainsAt, position)))
                let chainBits := or(leafBits, shl(32, chain))
                let last := or(chainBits, 15)
                for { let adrs := or(chainBits, digit) } lt(adrs, last) { adrs := add(adrs, 1) } {
                    mstore(0x20, adrs)
                    mstore(0x40, keccak256(0x00, 0x60))
                }
                mstore(add(0xc0, position), mload(0x40))
            }

            function refuse() {
                mstore(0x00, 0)
                return(0x00, 0x20)
            }

            if iszero(eq(sig.length, 2500)) { refuse() }
            let index := shr(224, calldataload(sig.offset))
            if gt(index, 1023) { refuse() }

            // digest = H(r || root || u256(index) || message)
            mstore(0x00, calldataload(add(sig.offset, 4)))
            mstore(0x20, root)
            mstore(0x40, index)
            mstore(0x60, message)
            let digest := keccak256(0x00, 0x80)

            // Chain hashes are H(pubSeed || ADRS || value), and CHAIN has type 0.
            mstore(0x00, pubSeed)
            let leafBits := shl(64, index)
            let chainsAt := add(sig.offset, 36)
            let checksum := 960
            for { let chain := 0 } lt(chain, 64) { chain := add(chain, 1) } {
                let digit := shr(252, digest)
                digest := shl(4, digest)
                checksum := sub(checksum, digit)
                finishChain(chain, digit, leafBits, chainsAt)
            }
            finishChain(64, shr(8, checksum), leafBits, chainsAt)
            finishChain(65, and(shr(4, checksum), 15), leafBits, chainsAt)
            finishChain(66, and(checksum, 15), leafBits, chainsAt)

            // leaf = H(pubSeed || ADRS(LEAF) || 67 chain ends)
            mstore(0x80, pubSeed)
            mstore(0xa0, or(shl(248, 1), leafBits))
            let node := keccak256(0x80, 2208)

            // Climb to the root. Bit z of the index says which side the node is on.
            let authAt := add(sig.offset, 2180)
            let place := index
            for { let height := 1 } lt(height, 11) { height := add(height, 1) } {
                let right := shl(5, and(place, 1))
                place := shr(1, place)
                mstore(0x20, or(or(shl(249, 1), shl(64, height)), shl(32, place)))
                mstore(add(0x40, right), node)
                mstore(sub(0x60, right), calldataload(authAt))
                authAt := add(authAt, 32)
                node := keccak256(0x00, 0x80)
            }

            mstore(0x00, eq(node, root))
            return(0x00, 0x20)
        }
    }
}
