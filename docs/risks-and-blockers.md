# Risks and blockers

Ranked by how likely each one is to sink the demo. Owner key: **P1** Chain · **P2** Rust/Device · **P3** Demo.

| ID | Risk / blocker | Severity | Mitigation | Owner |
|----|----------------|----------|------------|-------|
| <a id="r1"></a>R1 | **XMSS verify gas.** In Foundry the verifier uses 68,537-87,297 gas, 77,726 on average over 32 signatures. The public 100,000 `MAX_VERIFY_GAS` limit is not a design target | Medium | Measure the whole VERIFY frame on the devnet, where about 125-129k gas is expected | P1 |
| R2 | **Solidity can't emit EIP-8141 opcodes** (`APPROVE` 0xaa, `TXPARAM` 0xb0, `SIGDATACOPY` 0xb5) | High | The account deploys six tiny helper contracts from fixed runtime code and calls them. The verifier uses no new opcodes, so Foundry can test it | P1 |
| R3 | **No local EVM supports frame txs** (anvil/hardhat) | High | Foundry tests use `vm.etch` stand-ins for the helpers with real signatures. Real frame behaviour is only tested on the devnet, with `scripts/frame-demo.sh` | P1 |
| R4 | **Devnet instability**: the frame tx wire format isn't settled (ethrex's chain moved 81410 → 8141 and was "upgraded to current spec"); faucets have broken | High | Pin the client version (`web3_clientVersion`) in `docs/devnet.md`. Cross-check the encoding with `@jaw.id/frametx-kit`. Keep a funded key and a recorded video. If possible, run our own node as a backup | P1 |
| R5 | **VERIFY frames can't write storage** | High | `validate()` only reads. It checks that frame 1 is a `spendIndex` of the same index, and that frame writes `nextIndex` as a SENDER frame | P1 |
| R6 | **No mature tooling for type-0x06**: viem PR #4486 closed (superseded by #5118, status unknown); there's no Rust/alloy support | Medium | Our own encoder in `qday-wallet`, tested against the published vectors | P2 |
| <a id="r7"></a>R7 | **Restore residual risk**: a lost device holding signed-but-unsent txs at pages ≥ `chainNext + 64` | Medium (security story) | The restored device's first tx pushes on-chain `nextIndex` past those pages. Document the "≤ 64 unsent" rule. Make skip-ahead a constant | P2 |
| R8 | **Flash counter can be rolled back** (dump and reflash) | Medium (honesty) | Write-ahead persist-before-sign. eFuse high-water mark as a stretch goal. Say on stage that the chain lock covers everything on-chain | P2 |
| R9 | **Device keygen time**: about 1M keccak256 calls for 1,024 leaves, expected to take about 3 minutes. Not yet measured on the device | Low–Med | Do it once on first boot with a progress screen, and keep the tree as a 64 KB cache in flash | P2 |
| R10 | **Device must decode the frame tx itself** (trust-what-you-sign) | Medium | Accept exactly three frames, with no command to sign a bare hash. Show a warning for anything that isn't a plain transfer or rotate | P2 |
| R11 | **Calldata cost**: 2,500 B signature priced at ~16 gas/byte ≈ 40k gas (outside VERIFY, but adds to the cost) | Low | Include it in the "numbers" slide | P1 |
| R12 | **Seed entry on the device** is slow to build | Low | Restore comes in over USB as a declared demo shortcut | P2 |
| R13 | **Team of 2–3 for a 5-role plan** | High | Merged roles and cut order (spec §11). Don't start the web UI until the device signs | all |
| R14 | Hardware failure on stage | Medium | Two identically flashed ESP32-S3 boards (also needed for the clone-attack scene) | P3 |
| R15 | **Pitch overclaims** (see fact-check items 6, 9, 15, 17) | Medium | Apply the corrections in [fact-check.md](fact-check.md#corrections-to-make-in-the-pitch) | P3 |
