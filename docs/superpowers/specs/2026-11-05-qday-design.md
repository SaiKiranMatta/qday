# QDay — Design Spec

> **Superseded.** Kept for history. The build now uses keccak256 instead of SHA-256 and is not RFC 8391
> compatible, the verifier is Solidity with inline assembly instead of a Yul project, the device has
> four keys instead of two, and the 100,000 gas limit is not a design target. See
> [the signature scheme](../../xmss-spec.md) and the [README](../../../README.md).

- **Date:** 2026-11-05 (ETHGlobal Mumbai, Nov 5–7, 2026)
- **Status:** Draft for team review
- **Source idea:** `QDay-Overview.pdf` (summarised in [../../overview.md](../../overview.md))
- **Fact-check and blockers:** [../../fact-check.md](../../fact-check.md), [../../risks-and-blockers.md](../../risks-and-blockers.md)

## 1. Goal

A hardware device (ESP32-S3) plus an EIP-8141 smart account that only moves funds when it gets a
valid **XMSS** signature. Two locks stop any one-time key ("page") from being reused:

1. a forward-only counter on the device, and
2. a `nextIndex` that the account keeps on-chain.

**Demo target:** an alt-mempool EIP-8141 devnet (chain id **1337**) where a device-signed frame
transaction moves real (testnet) ETH, a cloned key cannot get a reused page accepted, and a
restored device resumes safely.

## 2. Decisions taken

| # | Decision | Choice | Notes |
|---|----------|--------|-------|
| D1 | Chain / tx format | EIP-8141 frame tx (type `0x06`), alt-mempool devnet, chain id 1337 | Team probe on 2026-11-05 confirmed the prefix layout below is accepted |
| D2 | Frame layout | `[pre_verify commit(idx), self_verify, SENDER execute]` | Lock 2 is enforced by `commit()`, not by binding to the protocol nonce |
| D3 | Lock 2 rule | `commit(idx)` requires `idx >= nextIndex`, then sets `nextIndex = idx + 1` | Gaps are allowed (wasted pages are fine) |
| D4 | Signature | XMSS, **RFC 8391 `XMSS-SHA2_10_256`** as the baseline (h=10 → 1,024 pages, n=32, w=16) | User requirement: the standard one |
| D5 | Gas budget | Whole validation prefix ≤ 100,000 gas (`MAX_VERIFY_GAS`), so the XMSS verifier gets ≤ ~80,000 | Verifier written in Yul |
| D6 | Fallback | If the verifier doesn't fit by **Fri 2026-11-06 04:00 IST** (after trying target-sum and w=4): switch to **B, sponsored execution** with the same verifier library | See §9 |
| D7 | Device | ESP32-S3, colour screen, 2 buttons, Rust `no_std` (esp-hal) | |
| D8 | Team | 2–3 people | Roles merged, see §11 |

### Open decision (needs the team before Fri 04:00)

**OD1: standard vs. fitting under 80k.** By our estimate, RFC 8391 `XMSS-SHA2_10_256` verification
costs about **1,800 SHA-256 precompile calls**: about 67 × 7.5 chain steps × 3 hashes, plus the
L-tree and the auth path. At ~200 gas per call before loop overhead, that comes to about
**0.4–0.8M gas**, 5–10× over the 80k budget. Neither **w=4** nor **target-sum WOTS** is an RFC 8391 /
NIST SP 800-208 parameter set. If the ladder in D6 is used, QDay no longer runs the standard scheme.
The team must choose one of:

- (a) keep RFC 8391 and go to fallback B straight away (unbounded gas in a DEFAULT frame), or
- (b) leave the standard to fit native VERIFY, and say on stage that the scheme is an
  "XMSS-family variant, not RFC 8391".

Phase 1 measures the real number first (task in the plan), so this choice rests on data.

## 3. Architecture

```
┌──────────── ESP32-S3 device ────────────┐        USB-serial        ┌──────── Laptop (host CLI, Rust) ───────┐
│ qday-xmss (no_std)  key tree cache       │ <──────────────────────> │ builds frame tx (type 0x06)            │
│ counter (flash WAL)  screen + 2 buttons  │   SignRequest / Sig       │ reads nextIndex from chain             │
│ frame-tx decoder: shows to / amount     │                           │ attaches XMSS sig, submits via RPC     │
└──────────────────────────────────────────┘                           └──────────────┬─────────────────────────┘
                                                                                      │ eth_sendRawTransaction
                                                                     ┌────────────────▼─────────────────────────┐
                                                                     │ EIP-8141 devnet (chain 1337, alt mempool)│
                                                                     │ QDayAccount (Yul)                        │
                                                                     │  frame0 VERIFY  commit(idx)              │
                                                                     │  frame1 VERIFY  self_verify → APPROVE    │
                                                                     │  frame2 SENDER  execute(to,value,data)   │
                                                                     └──────────────────────────────────────────┘
```

### Units

| Unit | Language | Purpose | Depends on |
|------|----------|---------|------------|
| `crates/qday-xmss` | Rust `no_std` | XMSS keygen, sign, verify; seed derivation from BIP39; parameter sets behind feature flags | `sha2` (hardware SHA on device via trait) |
| `crates/qday-frame` | Rust `no_std` | Encode/decode an EIP-8141 type-0x06 tx; `compute_sig_hash`; decode QDay `execute` calldata for display | `alloy-rlp`, `tiny-keccak` |
| `crates/qday-proto` | Rust `no_std` | Device ↔ host messages (length-prefixed CBOR) | `minicbor` |
| `host/` (`qday` CLI) | Rust | `init`, `address`, `send`, `status`, `restore-sync`, `clone-attack` demo commands | the three crates above, `alloy` RPC |
| `firmware/` | Rust (esp-hal) | Keys, counter, screens, buttons, sign flow | `qday-xmss`, `qday-frame`, `qday-proto` |
| `contracts/` | Yul (+ Solidity tests for pure parts) | `XmssVerifier` (pure, no new opcodes) and `QDayAccount` (uses `verbatim` for `APPROVE`/`TXPARAM`/`SIGDATACOPY`) | solc `--strict-assembly` |
| `web/` | TS (Vite) | Wallet view + "Q-Day simulator" | viem (read-only) |
| `vectors/` | JSON | Shared test vectors from the RFC reference implementation and `qday-xmss` | — |

**Why one Rust core:** the device and the laptop run the *same* `qday-xmss` and `qday-frame` code, so
they cannot disagree. The contracts are checked against the same `vectors/`.

## 4. Signature scheme details

- **Baseline:** RFC 8391 `XMSS-SHA2_10_256` (OID 0x00000001). Signature = idx (4 B) ‖ r (32 B) ‖
  67 × 32 B WOTS+ ‖ 10 × 32 B auth = **2,500 bytes**. Public key = OID ‖ root ‖ PUB_SEED.
- **Conformance:** `qday-xmss` must reproduce the signatures of the reference implementation
  (`github.com/XMSS/xmss-reference`) for fixed seeds. Those become `vectors/rfc8391/*.json`.
- **Key derivation (QDay-specific, not standardised):** 24-word BIP39 mnemonic → BIP39 seed (64 B) →
  `HKDF-SHA256(salt="QDay-XMSS-v1", info="book"‖u32(book))` → `SK_SEED ‖ SK_PRF ‖ PUB_SEED`
  (3 × 32 B). The signature format stays RFC-conformant; only seed generation is ours. This goes in the README.
- **Variants** (feature flags, OD1 only): `w4` (Winternitz w=4) and `target-sum` (WOTS with a
  fixed checksum target). Both are clearly marked **non-standard**.

## 5. On-chain account (`QDayAccount`)

### Storage

| Slot | Field |
|------|-------|
| 0 | `root` (bytes32) |
| 1 | `pubSeed` (bytes32) |
| 2 | `nextIndex` (uint32) ‖ `pendingIndex` (uint32) ‖ `book` (uint32) |

### Frames of a QDay transaction

| # | Mode | Target | Data | Does |
|---|------|--------|------|------|
| 0 | VERIFY (pre_verify) | account | `commit(idx)` | `require(idx >= nextIndex && idx < RESERVED)`; `nextIndex = idx+1`; `pendingIndex = idx` |
| 1 | VERIFY (self_verify) | account | — | `SIGDATACOPY` the ARBITRARY signature; check its idx field `== pendingIndex`; verify XMSS over `TXPARAM_SIG_HASH` against `root`/`pubSeed`; `APPROVE(EXECUTION_AND_PAYMENT)` |
| 2 | SENDER | account | `execute(to, value, data)` | performs the call |

- The signature entry uses the `ARBITRARY` scheme with an **empty `msg`**. Its raw bytes are left out
  of `compute_sig_hash` (only the length goes in), so signing the sig hash is well-defined.
- The sig hash covers frame 0's `idx` and frame 2's call, so the signature binds the page number and
  the payment together.
- If frame 1 fails, the whole tx is invalid and frame 0's write never lands. A bad signature cannot
  advance `nextIndex`.
- **To verify in Phase 0 (P0-probe):** the alt mempool allows frame 0's SSTORE to `tx.sender` storage.
  The team says this was confirmed on Nov 5. The probe tx hash goes in `docs/devnet.md`.

### Special operations

- **Rotate (new cheque book):** page `1023` (`RESERVED`) is used only for
  `rotate(newRoot, newPubSeed)`. It sets `book += 1` and `nextIndex = 0`. The device refuses normal
  transactions once its counter reaches `1023 - 8` and shows "rotate now".
- **Skip (restore):** `commit(idx)` already allows any `idx >= nextIndex`. A restored device's first
  transaction lands at `chainNext + 64`, which invalidates on-chain every unsent signature the lost
  device made below that point.

### Deployment

Deterministic CREATE2 factory, with `salt = keccak(root ‖ pubSeed)`. The old ECDSA wallet funds the
address once. No ECDSA key has authority over the account. It is **not upgradeable**.

## 6. Device firmware

- **Provisioning (demo):** the mnemonic is generated on the device and shown once. Restore is done by
  entering words with the two buttons (word list scroller). *Demo shortcut if short on time:* restore
  via serial, and say so on stage.
- **Key tree:** built once at provisioning (~3.4M SHA-256 calls on the hardware SHA engine; target
  < 2 min). The 2,047 nodes are cached (65,504 B) in a flash partition. Signing computes one WOTS+
  signature plus the auth path read from the cache (target < 100 ms).
- **Lock 1, the counter:** a write-ahead record in a dedicated flash sector:
  `persist(counter+1)` → read back → only then sign with page `counter`. It never decrements.
  `SyncIndex(chainNext)` sets `counter = max(counter, chainNext + 64)`.
  *Honest limit:* someone who dumps and reflashes the flash image can roll it back. Real anti-rollback
  needs a secure element (ATECC608 monotonic counter) or eFuse bits. Stretch goal: burn one eFuse bit
  per 4 pages as a high-water mark (256 user bits = 1,024 pages).
- **What you see is what you sign:** the device decodes the frame tx itself (`qday-frame`), checks that
  frames 0–2 have the expected shape and that frame 0's `idx` equals the page it is about to use,
  shows `to`, `value` (ETH), chain id, page `n/1024`, and computes the sig hash itself. It never
  signs a hash the host supplies.
- **Refusals (shown on screen):** wrong chain id, unexpected frame shape, `idx` ≠ the device's next
  page, counter at the reserved limit.

## 7. Host ↔ device protocol (`qday-proto`)

Length-prefixed CBOR over USB-CDC.

| Request | Response |
|---------|----------|
| `GetInfo` | `{ oid, root, pub_seed, book, counter, fw_version }` |
| `SyncIndex { chain_next }` | `{ counter }` |
| `Sign { chain_id, tx_unsigned_rlp }` | `{ idx, signature }` or `Rejected { reason }` (after the user presses Approve or Reject) |
| `Rotate { }` | signs the reserved page for the next book |

## 8. Lock analysis (why the two locks are enough)

| Threat | Stopped by |
|--------|------------|
| Device signs two different messages on one page | Lock 1 (persist-before-sign) |
| Cloned key signs a page already used on-chain | Lock 2 (`idx >= nextIndex`) |
| Host lies about `chainNext` (too low) to a restored device | Harmless. Pages below the true `nextIndex` are rejected on-chain, so a forgery on them is useless |
| Host lies (too high) | Wastes pages only |
| Lost device holds signed-but-unsent txs above `chainNext + 64` | **Residual risk.** Documented. The restore flow lands its first tx, pushing `nextIndex` past everything signed below. Recommend users never pre-sign more than 64 txs |
| Bad signature advances `nextIndex` | Not possible. A failed VERIFY makes the whole tx invalid |
| Flash rollback on a stolen device | Not covered in the hackathon build (see §6); the chain lock still covers anything already on-chain |

## 9. Fallback B: sponsored execution (only if D6 triggers)

Frames: `[VERIFY sponsor (ECDSA payer, cheap), DEFAULT → QDayAccount.execute(idx, call, xmssSig)]`.
`execute` verifies XMSS over `keccak(chainId ‖ account ‖ idx ‖ call)` with unbounded gas, checks and
stores `idx >= nextIndex`, then makes the call. Vault funds still have no ECDSA authority. A quantum
attacker could only take the sponsor's gas money. Same `XmssVerifier`, same device code; only the
message the device signs changes.

## 10. Testing

- `qday-xmss`: RFC reference vectors, sign/verify round trips, one-page-reuse forgery test (shows *why*
  Lock 1 matters), property tests (any single bit flip makes verify fail).
- `XmssVerifier` (pure Yul): run under Foundry against `vectors/`, with gas snapshots. **This is the
  number OD1 depends on.**
- `QDayAccount`: integration tests on the devnet only (anvil doesn't support EIP-8141): happy path,
  reused idx rejected, idx below nextIndex rejected, bad sig → tx invalid, rotate.
- Firmware: host-side unit tests of the sign flow with the counter mocked; a device-in-the-loop test
  through `qday status` / `qday send`.
- Cross-check the type-0x06 encoding against `@jaw.id/frametx-kit` (TS) on fixed fixtures.

## 11. Team and phases (2–3 people)

- **P1 — Chain:** `contracts/`, devnet tooling, gas work (owns OD1).
- **P2 — Rust/Device:** `qday-xmss`, `qday-frame`, `qday-proto`, `host/`, `firmware/`.
- **P3 — Demo** (or split between P1 and P2 if 2 people): `web/`, Q-Day simulator, clone/restore demo
  scripts, video, submission.

| Phase | Done when | Checkpoint (IST) |
|-------|-----------|------------------|
| 0 Spec + probe | This doc approved; devnet RPC and probe tx recorded | Thu 23:30 |
| 1 Signature core | `qday-xmss` passes RFC vectors; Yul verifier verifies vectors; **gas measured** | Fri 02:00 |
| OD1 gate | Verifier ≤ 80k (native) **or** switch to fallback B | **Fri 04:00** |
| 2 On-chain account | Laptop-signed tx accepted on chain 1337 | Fri 12:00 |
| 3 Device | Same tx signed by the ESP32-S3 | Fri 20:00 |
| 4 Safety | Clone attack refused by device and chain; restore + skip-ahead works | Sat 04:00 |
| 5 Demo | Web view, simulator, video, three rehearsals | Before submission (confirm deadline) |

**Cut order if late:** eFuse high-water mark → button-entry restore (use serial) → rotate → web
wallet (use CLI on stage). Never cut Lock 1, Lock 2, or the on-device display.

## 12. Out of scope

Mainnet, audits, multiple accounts per device, tokens other than ETH, a secure element, on-device
entropy certification, Ethereum consensus/validator keys.
