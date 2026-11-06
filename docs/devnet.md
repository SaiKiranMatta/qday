# Devnet notes

What the laptop-key run on 2026-11-06 ran against. The device has not been on this chain yet.

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

## Public reference network (not our target)

ethrex hegota-testnet: chain id 8141, RPC `https://rpc1.privacy.ethrex.xyz`, explorer
`https://dora.privacy.ethrex.xyz`. Enforces the public `MAX_VERIFY_GAS = 100,000`.
