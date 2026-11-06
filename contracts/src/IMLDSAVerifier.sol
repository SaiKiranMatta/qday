// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

/// @notice The ML-DSA-44 verifier in `contracts/mldsa`, which is built separately because it needs
///         a newer compiler. Returning false and reverting both mean not accepted.
interface IMLDSAVerifier {
    function verify(address pkBlob, bytes calldata message, bytes calldata signature)
        external
        view
        returns (bool accepted);
}
