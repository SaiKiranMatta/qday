# Plans roadmap

> **Superseded.** Kept for history. The build now uses keccak256 instead of SHA-256 and is not RFC 8391
> compatible, the verifier is Solidity with inline assembly instead of a Yul project, the device has
> four keys instead of two, and the 100,000 gas limit is not a design target. See
> [the signature scheme](../../xmss-spec.md) and the [README](../../../README.md).

Each plan produces working, testable software on its own. A plan is written only once its inputs
exist, because the later plans depend on the OD1 gas gate (spec §2).

| # | Plan | Status | Written when | Covers spec sections |
|---|------|--------|--------------|----------------------|
| 1 | [Phase 0–1: Signature core and gas gate](2026-11-05-phase1-signature-core.md) | **Ready** | now | §2 D1/D4/D5, §4, §10 (xmss + verifier) |
| 2 | On-chain account + host CLI on devnet 1337 (`qday-frame`, `QDayAccount` Yul with `verbatim`, CREATE2 factory, `qday send/status`) | Blocked on Plan 1 Task 9 | after the Fri 04:00 OD1 gate (native vs fallback B changes the frames and the signed message) | §3, §5, §9, §10 (account) |
| 3 | Device firmware on ESP32-S3 (`qday-proto`, BIP39 → HKDF seed derivation, tree cache, flash WAL counter, frame-tx display, approve/reject) | Blocked on Plan 2's frame format | Fri ~12:00 | §4 key derivation, §6, §7 |
| 4 | Safety features (restore + skip-ahead 64, clone-attack demo, rotate on page 1023, eFuse high-water mark stretch, page-reuse forgery test) | Blocked on Plan 3 | Fri ~20:00 | §5 special ops, §6 counter, §8 |
| 5 | Demo (web wallet view, Q-Day simulator, scripts, video, submission) | Can start early on mocks | any time P3 is free | §11 phase 5, fact-check corrections |

Known gaps tracked into later plans:
- BIP39/HKDF key derivation (spec §4): Plan 3.
- One-page-reuse forgery test (spec §10): Plan 4. It needs a search over `r`, so it isn't a quick unit test.
- Hardware SHA engine behind a trait for `qday-xmss`: Plan 3. Plan 1 uses the `sha2` crate.
