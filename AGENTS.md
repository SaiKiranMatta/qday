# AGENTS.md

A guide for AI agents and reviewers working in this repository: what QDay claims, where each claim
lives in the code, and the commands that check it. Humans should start with the [README](README.md).

## What this is

QDay is a hardware wallet for Ethereum that signs with hash-based (XMSS-style) signatures instead of
ECDSA. An ESP32-S3 device holds the keys and signs; an EIP-8141 smart account verifies the signatures
on-chain and has no ECDSA key at all. A web wallet talks to the device over Web Serial. It runs on an
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
| It works end to end on a real chain | Hosted devnet (`deploy/coolify`), wallet (`web/`) | Open [qday.unitlabs.xyz](https://qday.unitlabs.xyz); measured transactions are in `docs/devnet.md` |

## Map

| Path | What |
|---|---|
| `crates/qday-xmss` | The signature scheme, `no_std` |
| `crates/qday-wallet` | Device logic without hardware: counter, key store, passcode, backup, USB protocol, frame transactions, token list. `no_std` |
| `crates/qday-ui` | Device screens and themes, rendered to PNG on the laptop by the same code the device runs |
| `crates/qday-cli` | The `qday` laptop tool, including device commands |
| `crates/qday-wasm` | The frame-transaction code compiled for the browser |
| `crates/qday-fontgen` | Builds the device's glyph tables |
| `firmware/` | The ESP32-S3 app (outside the Cargo workspace) |
| `contracts/` | Foundry: verifier, account, factory |
| `web/`, `packages/` | The web wallet; the Web Serial protocol (`packages/device`); the WebAssembly wrapper (`packages/frame`) |
| `deploy/coolify`, `deploy/web` | The hosted devnet and the hosted wallet |
| `tools/devnet` | Local devnet, deploy scripts and the smoke test |
| `docs/` | Spec, overview, devnet measurements, fact-check, risks, roadmap |

## Commands

```bash
cargo test --workspace --locked                       # every Rust crate
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
cd contracts && forge test                            # the contracts
pnpm install && pnpm -r typecheck && pnpm -r test     # the web wallet and its packages
cargo run -p qday-ui --example preview                # every device screen, to docs/screens
```

CI (`.github/workflows/ci.yml`) runs the Rust and contract checks on every pull request. The firmware
needs the ESP toolchain (`firmware/README.md`) and is built separately.

## What is not real or not done

- The demo's quantum attacker is simulated. The signatures, counter and on-chain checks are real.
- EIP-8141 is a draft. QDay runs only on devnets.
- The ESP32-S3 is not a certified secure element, and its flash counter can be rolled back by
  re-flashing (`docs/risks-and-blockers.md`, R8). The chain's `nextIndex` still refuses reuse.
- The scheme is QDay's own keccak256 variant of XMSS, not RFC 8391 (`docs/xmss-spec.md`).
- Not audited.

See `docs/ROADMAP.md` for what comes next and `docs/fact-check.md` for every external claim checked
against its source.

## Working here

- Never commit `.env`: it holds the team key and the Coolify token.
- The `no_std` crates (`qday-xmss`, `qday-wallet`, `qday-ui`) run on the device; keep them free of
  `std` and heap-heavy code.
- Contract changes move the deployed addresses: regenerate `deploy/coolify/deployer/initcode` with
  its `export-initcode.sh` and update `web/src/lib/chain.ts` and `web/src/lib/tokens.ts`.
- Commits follow Conventional Commits.
