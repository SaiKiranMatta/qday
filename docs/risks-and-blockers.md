# Risks and blockers

Ranked by how likely each one is to sink the demo. Owner key: **P1** Chain · **P2** Rust/Device · **P3** Demo.

| ID | Risk / blocker | Severity | Mitigation | Owner |
|----|----------------|----------|------------|-------|
| <a id="r1"></a>R1 | **XMSS verify doesn't fit ≤ 80k gas.** RFC 8391 SHA2_10_256 ≈ 1,800 SHA-256 precompile calls (est. 0.4–0.8M gas) vs the 100k `MAX_VERIFY_GAS` prefix budget | **Blocker** | Measure first (Phase 1). Ladder: hand-tuned Yul → target-sum → w=4 → **fallback B by Fri 04:00**. Note that steps 2–3 leave RFC 8391 (open decision OD1 in the spec) | P1 |
| R2 | **Solidity can't emit EIP-8141 opcodes** (`APPROVE` 0xaa, `TXPARAM` 0xb0, `SIGDATACOPY` 0xb5) | High | Write the account in Yul objects using `verbatim`, compiled with `solc --strict-assembly`. Keep `XmssVerifier` free of new opcodes so Foundry can test it | P1 |
| R3 | **No local EVM supports frame txs** (anvil/hardhat) | High | Pure verifier tested in Foundry; account glue tested only on devnet 1337. Write a one-command deploy + smoke script | P1 |
| R4 | **Devnet instability**: the frame tx wire format isn't settled (ethrex's chain moved 81410 → 8141 and was "upgraded to current spec"); faucets have broken | High | Pin the client version (`web3_clientVersion`) in `docs/devnet.md`. Cross-check the encoding with `@jaw.id/frametx-kit`. Keep a funded key and a recorded video. If possible, run our own node as a backup | P1 |
| R5 | **Pre-verify SSTORE** (`commit(idx)` in frame 0) is not allowed by the *public* EIP-8141 rules (SSTORE banned in the validation prefix); it relies on alt-mempool rules | High | The team probed it on 2026-11-05. Record the probe tx. If the devnet's rules change, move to fallback B (counter written in a DEFAULT frame) | P1 |
| R6 | **No mature tooling for type-0x06**: viem PR #4486 closed (superseded by #5118, status unknown); there's no Rust/alloy support | Medium | Our own `qday-frame` crate (RLP + `compute_sig_hash`), tested against frametx-kit fixtures | P2 |
| <a id="r7"></a>R7 | **Restore residual risk**: a lost device holding signed-but-unsent txs at pages ≥ `chainNext + 64` | Medium (security story) | The restored device's first tx pushes on-chain `nextIndex` past those pages. Document the "≤ 64 unsent" rule. Make skip-ahead a constant | P2 |
| R8 | **Flash counter can be rolled back** (dump and reflash) | Medium (honesty) | Write-ahead persist-before-sign. eFuse high-water mark as a stretch goal. Say on stage that the chain lock covers everything on-chain | P2 |
| R9 | **Device keygen time**: ~3.4M SHA-256 calls for 1,024 leaves | Low–Med | Use the ESP32-S3 hardware SHA engine and do keygen once at provisioning. Fallback: generate the tree on the laptop for the demo device and say so | P2 |
| R10 | **Device must decode the frame tx itself** (trust-what-you-sign) | Medium | Fix the frame shape to exactly 3 frames; decode only `execute(to,value,data)`; refuse anything else | P2 |
| R11 | **Calldata cost**: 2,500 B signature priced at ~16 gas/byte ≈ 40k gas (outside VERIFY, but adds to the cost) | Low | Include it in the "numbers" slide | P1 |
| R12 | **Two-button mnemonic entry** is slow to build | Low | Serial restore as a declared demo shortcut | P2 |
| R13 | **Team of 2–3 for a 5-role plan** | High | Merged roles and cut order (spec §11). Don't start the web UI until the device signs | all |
| R14 | Hardware failure on stage | Medium | Two identically flashed ESP32-S3 boards (also needed for the clone-attack scene) | P3 |
| R15 | **Pitch overclaims** (see fact-check items 6, 9, 15, 17) | Medium | Apply the corrections in [fact-check.md](fact-check.md#corrections-to-make-in-the-pitch) | P3 |

## Decision gates

| When (IST) | Gate | If it fails |
|------------|------|-------------|
| Thu 23:30 | Devnet probe recorded, spec approved | Stop and fix before coding |
| Fri 02:00 | Gas measured for RFC-XMSS verifier | Start target-sum / w=4 in parallel |
| **Fri 04:00** | Prefix ≤ 100k on devnet | **Switch to fallback B** (spec §9) |
| Fri 20:00 | Device signs an accepted tx | Demo with laptop signer + device mock; say so |
| Sat 04:00 | Clone + restore scenes work | Cut rotate and the web wallet |
