# Notice

This folder is a vendored copy of `evm-ml-dsa-verifier`, commit `cca262b`, under its MIT licence
(see [LICENSE](LICENSE)). It's a separate Foundry project because it needs solc 0.8.30, `via_ir` and
`evm_version` osaka.

## Changes from upstream

1. **`src/Ntt.sol`, `src/InvNtt.sol`:** every `mstore(<x>, gas())` is now `mstore(<x>, 0)`. The
   mempool bans the GAS opcode during validation, and these were profiling writes. There were 18
   such lines, which compile to 9 standalone GAS opcodes. `staticcall(gas(), ...)` is unchanged.
2. **`test/QDayMempoolScan.t.sol`:** new. It scans the deployed bytecode and fails on any GAS not
   directly before a CALL-family opcode, or on any other opcode the mempool bans.

3. **`src/MLDSA44InlineVerifier.sol`:** new, a copy of `src/MLDSA44Verifier.sol` that takes `tr` and
   `t1hat` from calldata and `Ahat` from a shared 16,385-byte data contract. Keys can then share one
   matrix and need no blob of their own. The checks themselves are unchanged.

`helpers/f1600_170.hex` is unchanged, because its code hash is pinned.

## What the patch breaks

**The patch breaks the anchor of formal obligation C16** (see `docs/SAFETY.md`, section 3.3). C16
slices the transforms into regions at the `mstore(add(PR, ...), gas())` markers, and those markers no
longer exist in `src/`. The proof-to-code link for the NTT is therefore not checked for this copy.

Those were the only instructions changed. The writes went to a caller-owned buffer, and read
nothing and branched on nothing, so they could not affect the verdict.

| Check | Upstream | This copy |
|---|---|---|
| `forge test` | 330 of 330 pass | 341 of 341 pass, with the 11 added here |
| Verdicts across those 330 tests | | Unchanged |
| `verify()` gas in `test_e2e_10` | 1,226,311 | 1,227,540 |
| Runtime size | 24,032 bytes | 24,163 bytes |
| `formal/run_checks.sh`: z3 suite | Pass | **Fails** on C16, and on the `META-IDS` count that follows from it |
| `formal/run_checks.sh`: hypotheses | Pass | **Fails**, because the docs' published test counts no longer match after the added test |
| `formal/run_checks.sh`: Lean axioms | Not run: Lean isn't installed | Not run |

These were measured with Forge 1.2.3, not the nightly the upstream README names.

To restore C16, first move its anchoring to another pinned feature of the source, as
`docs/SAFETY.md` says.
