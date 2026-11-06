# AGENTS.md

A guide for AI agents and reviewers working in this repository: what QDay claims, where each claim
lives in the code, and the commands that check it. Humans should start with the [README](README.md).

## What this is

QDay is a hardware wallet for Ethereum that signs with post-quantum signatures instead of ECDSA: a
hash-based XMSS-style scheme, or ML-DSA-44 (FIPS 204), chosen on the device. An ESP32-S3 device holds
the keys and signs; an EIP-8141 smart account per scheme verifies the signatures on-chain and has no
ECDSA key at all. Private payments go to a post-quantum stealth address: one-time ML-DSA keys derived
with X-Wing, held in a shared pool. A web wallet talks to the device over Web Serial. It runs on an
EIP-8141 devnet (chain 1337) that QDay hosts; EIP-8141 is not on mainnet.

Built at ETHGlobal Mumbai (November 2026). A prototype: not audited, not for real funds.

## Claims and evidence

| Claim | Where it is implemented | How to check |
|---|---|---|
| Signatures are hash-based over keccak256, 2,500 bytes, 1,024 one-time keys per tree | `crates/qday-xmss`, spec in `docs/xmss-spec.md` | `cargo test -p qday-xmss` (includes the test vectors in `tests/tree_vectors.rs`) |
| The chain verifies them in Solidity, 68,537 to 87,297 gas per signature | `contracts/src/XMSSVerifier.sol` | `cd contracts && forge test` (real signatures from `test/fixtures/signatures.json`) |
| The account has no ECDSA key; only a valid QDay signature authorises a transaction | `contracts/src/QDayFrameAccount.sol` (`validate`) | `forge test --match-contract QDayFrameAccount` |
| A one-time key is never used twice, by two locks | Device: `Wallet::sign` advances the counter before signing (`crates/qday-wallet/src/wallet.rs`, `counter.rs`). Chain: `spendIndex` reverts with `KeyAlreadyUsed` below `nextIndex` (`QDayFrameAccount.sol`) | `cargo test -p qday-wallet`; the reuse refusal on a live chain is recorded in `docs/devnet.md` |
| The device decodes every transaction itself and refuses any other shape | `FrameTx::decode_unsigned` re-encodes and compares byte for byte (`crates/qday-wallet/src/frame.rs`) | `cargo test -p qday-wallet --test frame_vectors --test robustness` |
| The device shows ETH and ERC-20 transfers in plain terms, with a warning for anything it can't name | `Shown::from_tx` (`crates/qday-ui/src/app.rs`), token list in `crates/qday-wallet/src/tokens.rs` | `cargo test -p qday-ui --test app` |
| Keys 1008 to 1023 only rotate to a new tree, at the same address | `RESERVED_FROM` in `wallet.rs`; `rotate` and `ReservedKey` in `QDayFrameAccount.sol` | `cargo test -p qday-wallet`, `forge test` |
| A restored device skips 64 keys past the chain's `nextIndex` | `RESTORE_SKIP` in `wallet.rs` | `cargo test -p qday-wallet` |
| The browser builds transactions with the device's own code | `crates/qday-wasm`, wrapped by `packages/frame` | `pnpm --filter @qday/frame test` (checks the WebAssembly output against the CLI byte for byte) |
| Accounts deploy themselves from the device's key | `contracts/src/QDayAccountFactory.sol` (EIP-1167 clones, CREATE2), `web/src/lib/useCreateAccount.ts` | `forge test --match-contract QDayAccountFactory` |
| The device, the chain and the browser agree on bytes | Contract tests verify signatures produced by `qday-xmss` (`contracts/test/fixtures/signatures.json`); `packages/frame` tests compare the WebAssembly codec with the CLI | `scripts/check-all.sh` |
| The device also signs ML-DSA-44, from the same seed, and switches schemes only after approval | `crates/qday-mldsa` (`KeySeed::derive`), `crates/qday-wallet/src/wallet.rs` (`Scheme`), `mlkey.rs` | `cargo test -p qday-mldsa`, `cargo test -p qday-wallet --test mldsa --test ml_frame` |
| The chain verifies ML-DSA-44 in Solidity, about 1.23M gas per signature | `contracts/mldsa/src/MLDSA44Verifier.sol`, vendored from Fireblocks' `evm-ml-dsa-verifier` with its GAS opcodes removed (`contracts/mldsa/NOTICE.md`) | `cd contracts/mldsa && forge test` (needs the Python reference from its README); `QDayMempoolScan.t.sol` fails on any opcode the mempool bans |
| An ML-DSA account accepts only a valid signature from its key, and re-checks its verifier and key by code hash | `contracts/src/QDayMlDsaAccount.sol`, `QDayMlDsaFactory.sol` | `forge test --match-contract 'QDayMlDsa(Account|Factory)Test'` |
| Private payments: a one-time key per payment from X-Wing; the view key finds payments and can't spend them | `crates/qday-wallet/src/stealth.rs`, `crates/qday-mldsa/src/stealth.rs` | `cargo test -p qday-mldsa --test stealth`, `cargo test -p qday-wallet --test stealth --test stealth_pay` |
| The pool pays a note once, for the whole note, to the address the device signed for | `contracts/src/QDayLatticePool.sol`, its verifier `contracts/mldsa/src/MLDSA44InlineVerifier.sol` | `forge test --match-contract QDayLatticePoolTest` |
| It works end to end on a real chain | Hosted devnet (`deploy/coolify`), wallet (`web/`), demos (`scripts/mldsa-demo.sh`, `scripts/stealth-demo.sh`) | Open [qday.unitlabs.xyz](https://qday.unitlabs.xyz); measured transactions are in `docs/devnet.md` |

## Map

| Path | What |
|---|---|
| `crates/qday-xmss` | The XMSS signature scheme, `no_std` |
| `crates/qday-mldsa` | ML-DSA-44 signing (the only crate that uses `fips204`) and the stealth one-time keys, `no_std` |
| `crates/qday-wallet` | Device logic without hardware: counter, key store, scheme switching, passcode, backup, USB protocol, frame transactions, stealth payments, token list. `no_std` |
| `crates/qday-ui` | Device screens and themes, rendered to PNG on the laptop by the same code the device runs |
| `crates/qday-cli` | The `qday` laptop tool, including device commands |
| `crates/qday-wasm` | The frame-transaction code compiled for the browser |
| `crates/qday-fontgen` | Builds the device's glyph tables |
| `firmware/` | The ESP32-S3 app (outside the Cargo workspace) |
| `contracts/` | Foundry: the XMSS verifier, both accounts and factories, the lattice pool |
| `contracts/mldsa` | The vendored ML-DSA-44 verifier and its inline variant: its own Foundry project (solc 0.8.30, `via_ir`, osaka) |
| `web/`, `packages/` | The web wallet; the Web Serial protocol (`packages/device`); the WebAssembly wrapper (`packages/frame`) |
| `deploy/coolify`, `deploy/web` | The hosted devnet and the hosted wallet |
| `tools/devnet` | Local devnet, deploy scripts (`deploy-qday.sh`, then `deploy-stealth.sh`) and the smoke test |
| `scripts/` | `check-all.sh`; `mldsa-demo.sh` and `stealth-demo.sh` run the new features from a laptop key or a device |
| `docs/` | Spec, overview, devnet measurements, fact-check, risks, roadmap |

## Commands

`scripts/check-all.sh` runs all of these (except the screen renders) and stops at the first failure.

```bash
cargo test --workspace --locked                       # every Rust crate
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
cd contracts && forge test                            # the contracts
cd contracts/mldsa && forge test                      # the vendored ML-DSA verifier (not in check-all or CI)
pnpm install && pnpm -r typecheck && pnpm -r test     # the web wallet and its packages
cargo run -p qday-ui --example preview                # every device screen, to docs/screens
```

CI (`.github/workflows/ci.yml`) runs the Rust and contract checks on every pull request. The firmware
needs the ESP toolchain (`firmware/README.md`) and is built separately.

## What is not real or not done

- The demo's quantum attacker is simulated. The signatures, counter and on-chain checks are real.
- EIP-8141 is scheduled for the Hegotá upgrade (since 27 August 2026), but its spec is still a draft
  and it is not on mainnet. QDay runs only on devnets.
- The ESP32-S3 is not a certified secure element, and its flash counter can be rolled back by
  re-flashing (`docs/risks-and-blockers.md`, R8). The chain's `nextIndex` still refuses reuse.
- The scheme is QDay's own keccak256 variant of XMSS, not RFC 8391 (`docs/xmss-spec.md`).
- The stealth one-time signer is an unreviewed variant of ML-DSA, and recipient privacy rests on ML-KEM
  ciphertexts not revealing their recipient, an assumption. Test networks only.
- On a devnet the dev key deploys ML-DSA accounts and relays private-payment claims (repaid from the
  note). The device shows a private payment's amount and pool, not its recipient.
- The vendored verifier's formal check C16 no longer anchors after the GAS patch
  (`contracts/mldsa/NOTICE.md`).
- Not audited.

See `docs/ROADMAP.md` for what comes next and `docs/fact-check.md` for every external claim checked
against its source.

## Working here

- Never let a one-time key index be signed twice. Anything that touches the counter, key storage or
  `spendIndex` must keep "advance first, then sign" on the device and "refuse below `nextIndex`" on
  the chain.
- Never commit `.env`: it holds the team key and the Coolify token.
- The `no_std` crates (`qday-xmss`, `qday-wallet`, `qday-ui`) run on the device; keep them free of
  `std` and heap-heavy code.
- Contract changes move the deployed addresses: regenerate `deploy/coolify/deployer/initcode` with
  its `export-initcode.sh` and update `web/src/lib/chain.ts` and `web/src/lib/tokens.ts`. This covers
  the ML-DSA and pool contracts too; `tools/devnet/deploy-stealth.sh` must give the same addresses.
- Only `crates/qday-mldsa` may use `fips204`, so a custom signer can replace it.
- The vendored verifier must stay free of the opcodes the mempool bans during validation;
  `contracts/mldsa/test/QDayMempoolScan.t.sol` checks it.
- Commits follow Conventional Commits.
