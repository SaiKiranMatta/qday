# QDay

**A quantum-safe hardware wallet for Ethereum.** An ESP32-S3 device signs with an **XMSS-style**
hash-based signature over keccak256, and an **EIP-8141** smart account accepts only those signatures.
Two locks (a device counter and an on-chain `nextIndex`) make sure no one-time key is ever reused.

> ETHGlobal Mumbai, Nov 5–7 2026. Hackathon prototype. Not audited, not for real funds. The demo's
> quantum attacker is simulated; the signatures, counter and on-chain checks are real.

## How it works

1. The laptop builds an EIP-8141 frame transaction with three frames: `validate()`,
   `spendIndex(index)`, and the transfer or `rotate(...)`.
2. The device decodes the transaction itself, shows the recipient and amount, and you press
   **Approve** on K4.
3. The device advances its counter, *then* signs with the one-time key that index gives it.
4. On-chain, `validate()` checks the signature and that the transaction spends the same index, then
   `spendIndex` moves `nextIndex` forward and the last frame runs.

The account has **no ECDSA key**. Restoring from the 24 backup words reads `nextIndex` from the chain
and skips ahead 64 keys. Keys 1008-1023 are reserved for rotating to a new tree at the same address.

## Status

Steps 1 to 6 of 8 are built and tested on the laptop, and the firmware compiles.

Nothing has run on the device yet, and the contracts have only run in Foundry and a plain EVM. Still to
do: a laptop-key transfer and rotation on a local EIP-8141 chain, then the on-device runs in steps 7
and 8.

## Repository layout

| Path | What |
|------|------|
| `crates/qday-xmss` | The signature scheme, `no_std` |
| `crates/qday-wallet` | Counter, key storage, passcode, backup, USB protocol, frame transactions, `no_std` |
| `crates/qday-ui` | Screens, soft keys and themes, `no_std`, renders to PNG on the laptop |
| `crates/qday-cli` | The `qday` laptop tool |
| `firmware/` | ESP32-S3 app, kept out of the host workspace |
| `contracts/` | Foundry: Solidity verifier with inline assembly, and the account |
| `scripts/` | Deploy, send and rotate |
| `hardware/` | The printable case |

## Docs

- [Signature scheme](docs/xmss-spec.md): the byte-exact definition and test vectors
- [Overview](docs/overview.md): the idea in two pages
- [Fact-check](docs/fact-check.md): what in the original pitch holds, and what to correct
- [Risks and blockers](docs/risks-and-blockers.md): ranked
- [Devnet notes](docs/devnet.md)
- [Screens](docs/screens): every screen in both themes
- [Original design spec](docs/superpowers/specs/2026-11-05-qday-design.md) and
  [plans](docs/superpowers/plans/README.md), kept for history. The build has moved on from them.

## Quick start

Run the laptop tests from the repository root:

```bash
cargo test --workspace
```

Run the contract tests:

```bash
cd contracts && forge test
```

Build the firmware:

```bash
cd firmware && source ~/export-esp.sh && cargo build --release
```

Render every screen in both themes to PNG:

```bash
cargo run -p qday-ui --example preview docs/screens
```

Deploy an account and send from it. `scripts/frame-demo.sh` explains its settings at the top:

```bash
scripts/frame-demo.sh deploy
```

The contract tests check real signatures from `contracts/test/fixtures/signatures.json`. To regenerate
them, run this from the repository root:

```bash
cargo run -p qday-xmss --example contract_fixtures > contracts/test/fixtures/signatures.json
```

To rebuild the font atlases from Inter, pass the font file to `scripts/build-fonts.py`.

## Key derivation note

The seed is the entropy of 24 BIP39 words. Each tree's secrets come from that seed and the tree
number with keccak256, as set out in the [signature scheme](docs/xmss-spec.md#keys). This is QDay's
own scheme, because no standard exists for it.
