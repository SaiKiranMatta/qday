# QDay

**A quantum-safe hardware wallet for Ethereum.** An ESP32-S3 device signs with **XMSS**
(RFC 8391, hash-based, post-quantum), and an **EIP-8141** smart account accepts only those signatures.
Two locks (a device counter and an on-chain `nextIndex`) make sure no one-time key is ever reused.

> ETHGlobal Mumbai, Nov 5–7 2026. Hackathon prototype. Not audited, not for real funds. The demo's
> quantum attacker is simulated; the signatures, counter and on-chain checks are real.

## How it works

1. The laptop builds an EIP-8141 frame tx: `[commit(idx), self_verify, execute(to, value, data)]`.
2. The device decodes the tx itself, shows the recipient and amount, and you press **Approve**.
3. The device advances its counter, *then* signs page `idx` with XMSS.
4. On-chain, `commit` enforces `idx >= nextIndex`, `self_verify` checks the XMSS signature over the tx
   sig hash, and `execute` sends the funds.

The account has **no ECDSA key**. Restoring from backup words reads `nextIndex` from the chain and
skips ahead 64 pages. Page 1023 is reserved to rotate to a new key book with the same address.

## Status

Phase 0–1 (signature core and gas gate). See the roadmap below.

## Repository layout (planned)

| Path | What |
|------|------|
| `crates/qday-xmss` | RFC 8391 XMSS-SHA2_10_256, `no_std` Rust (shared by laptop and device) |
| `crates/qday-frame`, `crates/qday-proto` | EIP-8141 tx encode/decode; device ↔ host protocol |
| `host/` | `qday` CLI |
| `firmware/` | ESP32-S3 firmware |
| `contracts/` | Yul XMSS verifier + `QDayAccount` (Foundry tests) |
| `vectors/` | Shared test vectors (RFC reference + QDay) |
| `web/` | Wallet view and Q-Day simulator |

## Docs

- [Overview](docs/overview.md): the idea in two pages
- [Fact-check](docs/fact-check.md): what in the original pitch holds, and what to correct
- [Risks and blockers](docs/risks-and-blockers.md): ranked, with decision gates
- [Design spec](docs/superpowers/specs/2026-11-05-qday-design.md)
- [Plans roadmap](docs/superpowers/plans/README.md) and the [Phase 0–1 plan](docs/superpowers/plans/2026-11-05-phase1-signature-core.md)
- [Devnet notes](docs/devnet.md)

## Quick start (once Phase 1 lands)

```bash
cargo test -p qday-xmss
tools/xmss-ref/build.sh && cargo test -p qday-xmss --test rfc_reference
contracts/scripts/build-yul.sh && (cd contracts && forge test -vv)
```

## Key derivation note

Signatures follow RFC 8391 byte for byte. Deriving the XMSS seeds from a BIP39 mnemonic
(HKDF-SHA256, salt `QDay-XMSS-v1`) is QDay's own scheme, because no standard exists for it.
