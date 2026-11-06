// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

/// @notice The ML-DSA-44 verifier that takes a key from calldata and the matrix from a shared data
///         contract. Returning false and reverting both mean not accepted.
interface IMLDSAInlineVerifier {
    function verifyInline(address matrix, bytes calldata key, bytes calldata message, bytes calldata signature)
        external
        view
        returns (bool accepted);
}
