# Devnet notes

What the runs on 2026-11-06 ran against.

| Field | Value |
|-------|-------|
| Chain id | 1337 |
| RPC URL | `http://127.0.0.1:14337/rpc` (mempool gateway), `http://127.0.0.1:8545` (ethrex) |
| Explorer | None |
| Funded key | The genesis premine key, kept outside the repository |
| `web3_clientVersion` | `ethrex/v23.0.0-eip8141-tracer-9aa3645d795945c64400f41a70aace0edfa7f383/x86_64-unknown-linux-gnu/rustc-v1.93.1` |
| Mempool rules | ERC-8421 sidecar, minimum stake 0.01 ETH |
| Frame transaction accepted | Yes, see below |

## Measured results

Laptop key, 2026-11-06. Frame transactions go through the gateway with `eth_sendFrameTransaction`, and
ethrex itself doesn't have that method. The gateway has no `eth_feeHistory`, so plain transactions use
legacy gas pricing.

| Transaction | Total gas | VERIFY frame |
|---|---|---|
| First transfer on a tree | 475,641 (291,981 after a rotation) | 120,755 |
| Repeat transfer | 188,979 to 189,015 | 115,625 |
| Rotation | 297,572 | 111,017 |

The first transfer on a tree costs more because it writes `nextIndex` from zero.

Refused by the chain, with a valid signature in each case:

| Attempt | Error |
|---|---|
| Reusing spent key 0 on tree 1 | `KeyAlreadyUsed` |
| Signing with key 5 of retired tree 0 | `BadSignature` |

The device signed its first transfer on this chain the same day: 0.0025 ETH, approved on K4 with the
passcode, mined with 468,765 gas in total and 113,915 in the VERIFY frame. The recipient gained exactly
that amount, and the device and the chain both moved to key 1.

It then rotated, all on the device:

| Step | Result |
|---|---|
| Build tree 1 | 99 seconds |
| Rotation | 316,334 gas, 129,827 in the VERIFY frame |
| Activate | The device restarted on tree 1, at the same address |
| Transfer from tree 1 | 0.004 ETH, 292,029 gas, 120,755 in the VERIFY frame |

## ML-DSA account

Laptop key, 2026-11-06, on the same chain. The sidecar's `MAX_VERIFY_GAS_STAKED_ENTITY` was raised from
1,000,000 to 10,000,000 first, because a verify costs about 1.25M.

| Transaction | Total gas | VERIFY frame |
|---|---|---|
| Keccak helper deployment (shared) | 33,632,238 | |
| Key blob deployment | 31,944,668 | |
| Account deployment | 5,365,324 | |
| First transfer | 1,492,592 | 1,248,350 |
| Repeat transfer | 1,309,131 | 1,248,489 |

Code costs about 1,555 gas per byte on this chain, not the usual 200, so deployment costs far more
than a standard EVM estimate.

A forged signature, a signature for another amount, a signature one byte short and a signature from
another key were all refused with `BadSignature`. A replay was refused with a nonce mismatch. The
gateway reports that replay as `prefix gas budget exceeded`, because it shows the node's public
mempool note ahead of the real error.

## Public reference network (not our target)

ethrex hegota-testnet: chain id 8141, RPC `https://rpc1.privacy.ethrex.xyz`, explorer
`https://dora.privacy.ethrex.xyz`. Enforces the public `MAX_VERIFY_GAS = 100,000`.

## Lattice pool

`scripts/stealth-demo.sh`, 2026-11-06, on the same chain, as plain transactions with the funding key
as the relayer. Both a laptop key and the device code were tested, the device code on simulated flash.
No real board was involved, and the signer is an unreviewed variant, so this is for test networks only.

| Transaction | Total gas |
|---|---|
| Helper deployment (once per chain) | 33,632,276 |
| Matrix deployment (once per chain) | 25,496,912 |
| Pay and announce | 162,152-162,212 |
| Spend, recipient already exists | 1,381,678-1,384,417 |

Two recipients shared one pool. Each one's scan found only their own payments, and a recipient couldn't
spend the other's note. A second spend of a note reverted, and a spend rejected on the device got no
signature. A spend worked in either signing mode and left the one-way counter alone.
