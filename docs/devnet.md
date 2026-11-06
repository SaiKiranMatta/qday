# Devnet notes

QDay runs on its own EIP-8141 devnet. The same chain setup runs in two places:

| | Hosted | Local |
|---|---|---|
| Node RPC | `https://rpc-qday.unitlabs.xyz` | `http://127.0.0.1:8545` |
| Gateway (frame transactions) | `https://gateway-qday.unitlabs.xyz/rpc` | `http://127.0.0.1:14337/rpc` |
| Wallet | [qday.unitlabs.xyz](https://qday.unitlabs.xyz) | `pnpm dev` |
| Start it | Always on ([deploy/coolify](../deploy/coolify)) | `tools/devnet/local/up.sh` |

| Field | Value |
|-------|-------|
| Chain id | 1337, 6-second slots |
| Clients | ethrex (`eip8141-tracer`) and Lighthouse, with the ERC-8421 gateway |
| Explorer | Built into the wallet: `/tx/<hash>` decodes frame transactions |
| Funded key | The devnet's premine key, public by design (`tools/devnet/local/up.sh`). Nothing of value lives here |
| Gateway verify budget | Raised to 10,000,000 gas (the public rule is 100,000, too little for an XMSS check) |
| Contracts | Factory `0xbCebAd3eCE45625983409B429406a5Dc8d38f876`, verifier `0xe0Ab78755c24867147473b12ffA430D24C1ed127`, USDC `0xd1108cBf3C5E7b44D7A58d4b2cdA492a0DDF6a72`, WBTC `0xb212346871a9a08494e165F5AeFe655f06A5B75F` (the same on every reset, through CREATE2) |
| Smoke test | `tools/devnet/smoke.sh`: ETH and token transfers, approve, transferFrom, overspend refused (14 checks, passing on both) |

## Signed on the device

Transactions the ESP32-S3 approved and signed from the web wallet, 2026-11-06, read back with the
built-in explorer.

| Transaction | Total gas | Note |
|---|---|---|
| Create the account | 464,530 | Deploys the account (an EIP-1167 clone) and pays for it from its own balance |
| Send 0.0001 ETH | 197,171 | VERIFY frame 123,577 |
| Send 29.5 USDC | 207,094 | VERIFY frame 111,607; ERC-20 `transfer` decoded and shown on the device |

## Measured results

First runs with a laptop key, 2026-11-06. Frame transactions go through the gateway with `eth_sendFrameTransaction`, and
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

## Public reference network (not our target)

ethrex hegota-testnet: chain id 8141, RPC `https://rpc1.privacy.ethrex.xyz`, explorer
`https://dora.privacy.ethrex.xyz`. Enforces the public `MAX_VERIFY_GAS = 100,000`.
