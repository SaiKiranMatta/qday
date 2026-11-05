# QDay — Overview

> Summary of `QDay-Overview.pdf` (team doc, ETHGlobal Mumbai, Nov 5–7 2026). Fact-check notes are
> linked inline; see [fact-check.md](fact-check.md).

**One line:** a small hardware device plus an Ethereum smart account. It signs with **XMSS**, a hash-only
signature that quantum computers can't forge, and makes reuse of a one-time key impossible.

## Problem

ECDSA keys fall to Shor's algorithm. Once an address has sent a transaction, its public key is public,
so on Q-Day the funds can be taken. 2026 estimates put the attack at ~1,200 logical qubits
([fact-check #1](fact-check.md)). If you wait, you'll be moving funds at the same moment as everyone else.

## What QDay is

| Part | Role |
|------|------|
| Device | ESP32-S3 with a screen and two buttons. Holds the XMSS key, shows what you sign, signs only on Approve |
| Account | EIP-8141 smart account. Accepts only a valid XMSS signature. No ECDSA key anywhere |

## The one catch: one-time keys

XMSS-SHA2_10_256 is a "cheque book" of **1,024 pages**. Signing two *different* messages on one page lets
an attacker forge. QDay has two locks:

1. **Device counter:** moves forward before every signature.
2. **Chain remembers:** the account's `commit(idx)` rejects `idx < nextIndex`.

**Restore rule:** a restored device reads `nextIndex` from the chain and skips ahead 64 pages.
**Rotation:** the last page registers a new book; the address stays the same.

## Solves / doesn't solve

Solves: quantum-safe custody today (on an EIP-8141 devnet; mainnet once Hegotá ships), safe use of
one-time keys, a calm migration, recovery without a company, trust in what you sign.

Doesn't solve: funds left in exposed addresses, validator keys, other people's exposed keys,
high-frequency use.

## Who it's for

Long-term holders, DAO treasuries, custodians, inheritance vaults, protocol admin keys,
hardware-wallet users.

## Demo (4 min)

| Time | Scene |
|------|-------|
| 0:00 | The threat: a simulated quantum "recovery" drains a normal wallet |
| 0:50 | QDay holds: the device approves a real transfer on the devnet |
| 1:50 | Clone attack: the second device refuses; a forced reuse is rejected by the chain |
| 2:50 | Recovery: a fresh device restores, reads the chain, skips ahead, signs |
| 3:30 | The numbers: measured gas vs other PQ options |

The quantum computer is simulated, and we say so. The signatures, counter and on-chain checks are real.

For the build design, see [superpowers/specs/2026-11-05-qday-design.md](superpowers/specs/2026-11-05-qday-design.md).
