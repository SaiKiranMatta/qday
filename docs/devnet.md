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
| Gateway verify budget | Raised to 10,000,000 gas (the public rule is 100,000, too little for an XMSS check at about 120,000, let alone ML-DSA at about 1.25M) |
| XMSS contracts | Factory `0xbCebAd3eCE45625983409B429406a5Dc8d38f876`, verifier `0xe0Ab78755c24867147473b12ffA430D24C1ed127`, USDC `0xd1108cBf3C5E7b44D7A58d4b2cdA492a0DDF6a72`, WBTC `0xb212346871a9a08494e165F5AeFe655f06A5B75F` |
| ML-DSA and private payments | ML-DSA factory `0x6aD8F805f02509bdf646406b9108dd3070E4174d`, verifier `0x20a8128F20FAF48A9A26Bb83c332582520100954`, pool `0x4eBfa044FA7a52CfDE7Fc1B6d8Fb3b4b0669f354`, and on hosted the staking registry `0xb9654f4208EE0809D212B66eD145A3CC07464857` ([all of them](../deploy/coolify/README.md)) |
| Addresses | The same on every reset, through CREATE2: `deploy/coolify` deploys them all; locally, `tools/devnet/deploy-qday.sh` then `deploy-stealth.sh` |
| Smoke test | `tools/devnet/smoke.sh`: ETH and token transfers, approve, transferFrom, overspend refused (14 checks, passing on both) |

## Hosted: ML-DSA and private payments

2026-11-07, on the hosted devnet, with laptop keys through `scripts/mldsa-demo.sh` and
`scripts/stealth-demo.sh`, after the hosted deployer had put the contracts above in place.

| Transaction | Total gas | VERIFY frame |
|---|---|---|
| ML-DSA key blob | 31,944,704 | |
| ML-DSA account, staking 0.012 ETH in the registry | 5,364,655 | |
| First ML-DSA transfer, 0.0125 ETH | 1,492,003 | 1,247,821 |
| Repeat ML-DSA transfer, 0.001 ETH | 1,309,213 | 1,248,535 |
| Private payment of 0.05 ETH | 162,188 | |
| Claim to a new address | 1,565,900 | |

The scan found the one payment, the new address received 0.0466 ETH (the note less the 0.0034 ETH
fee), and a second claim of the same note reverted. The hosted gateway reads no stakes and has one
verify budget, so the ML-DSA check fits it without the stake-specific raise described below.

## Signed on the device

Transactions the ESP32-S3 approved and signed from the web wallet, 2026-11-06, on the local devnet,
read back with the built-in explorer.

| Transaction | Total gas | Note |
|---|---|---|
| Create the account | 464,530 | Deploys the account (an EIP-1167 clone) and pays for it from its own balance |
| Send 0.0001 ETH | 197,171 | VERIFY frame 123,577 |
| Send 29.5 USDC | 207,094 | VERIFY frame 111,607; ERC-20 `transfer` decoded and shown on the device |

## Measured results

First runs with a laptop key, 2026-11-06, on the team's local devnet (ethrex `eip8141-tracer` and the
ERC-8421 sidecar, minimum stake 0.01 ETH). The sections after this one ran on that chain too, unless
they say otherwise. Frame transactions go through the gateway with `eth_sendFrameTransaction`, and
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

## Web wallet

Tested on 2026-11-07 in a browser, on the same chain. The page was the real web app. A test-only Web
Serial port connected it to the real app and wallet code on simulated flash. No board was involved.

An XMSS account made by the factory is unstaked, and the sidecar's budget for an unstaked sender is 100,000
gas. `tools/devnet/local/up.sh` raises that. The XMSS rows below went through a throwaway second gateway with
that raise. The chain's own gateway and miner were then raised to 10,000,000 too. Through them, a new XMSS
account deployed itself at 467,878 gas and an existing one sent at 191,957 gas. The change is a file copied into those two containers, so recreating
them loses it. The ML-DSA account is staked, so it never needed this.

| Step | Account | Result |
|---|---|---|
| Create, ETH transfer, USDC transfer | XMSS | Mined |
| Private payment | XMSS | Mined, 360,325 gas |
| Claim to a new address | XMSS | Mined, 1,567,105 gas |
| Create (dev key pays), ETH transfer | ML-DSA | Mined |
| Private payment | ML-DSA | Mined, 1,448,125 gas |
| Rejected on the device, then claimed to the account | ML-DSA | Refused, then mined at 1,382,451 gas |

The private payments went to the device's own private address, which is the same in both schemes.

To run the wallet against a devnet not started by `up.sh`, set `VITE_QDAY_DEV_KEY` to a funded key. ML-DSA
accounts need `VITE_QDAY_STAKING_REGISTRY` on any devnet but the hosted one, which has its own. `tools/devnet/deploy-stealth.sh` deploys what
ML-DSA accounts and private payments share, after `tools/devnet/deploy-qday.sh`.

The wallet's limits:

- Private payments carry ETH only, and ML-DSA accounts can't send tokens.
- Only a network with a dev key can use private payments or create ML-DSA accounts, because the dev key
  submits claims and deploys the accounts. That means QDay's own devnets, local and hosted.
- The device shows the amount and the pool for a private payment, not the recipient. The address the person
  pasted is what decides who gets paid, so a page that swaps it can't be caught on the device.
- The view key stays in the page's memory, so each visit asks the device for it again.

## Real board

On 2026-11-07 the board ran the firmware with its framebuffer in PSRAM. Everything below went through the
chain's own gateway, signed on the board with its keys and passcode.

| Measure | Framebuffer in internal RAM | Framebuffer in PSRAM |
|---|---|---|
| Stack free, idle | 25,252 bytes | 172,780 bytes |
| Full redraw, middle timing per screen | 59 to 98 ms | 64 to 104 ms |
| Worst redraw | 127.6 ms | 133.6 ms |
| Frame with no changes | 4.2 ms | 5.8 ms |

| Transaction | Scheme | Total gas |
|---|---|---|
| Create account | XMSS | 471,406 |
| 0.001 ETH transfer | XMSS | 193,679 |
| 0.0125 ETH transfer | ML-DSA | 1,308,782 |
| 0.002 ETH transfer, after switching back | XMSS | 191,993 |
| 0.05 ETH private payment, from the web wallet | XMSS | 360,325 |
| Claim to a new address, from the web wallet | Stealth signer | 1,566,169 |

Switching to ML-DSA and back left the XMSS key and counter unchanged. The board and the chain agreed on the
next signature at each check.

The last two rows ran in Chrome over real Web Serial, with the board sharing its private address and view
key. The new address received 0.0466 ETH: the note less the 0.0034 ETH fee. A private payment from an
ML-DSA account has only run on simulated flash.

## Demo wallet

**Try without a device** on the connect screen opens a wallet with its keys in the browser, so people can try
QDay without a board. It's XMSS only, for test ETH.

Tested on 2026-11-07 in a browser against the chain's own gateway, with no device connected: create, ETH
transfer, a rejection, private payment, claim, reload, seed export, delete and restore. A wallet opens in about
half a second.

What stands in for the device:

- **One-time keys:** before each signature it reads the next key from the chain and from browser storage, and
  uses the later one. It saves the new counter, and reads it back, before it signs. With the local counter
  deleted, it picked up the right key from the chain.
- **Several tabs:** signing takes a Web Lock, so tabs take turns.
- **Networks:** it refuses to open or sign unless the chain ID is 1337.
- **Approval:** a page dialog copies the device's review screen, and says what real hardware adds.

Its limits:

- **Keys aren't protected.** The seed sits unencrypted in browser storage, so anything that reaches the page can
  take it. The approval dialog is on the same page, so it proves nothing against a bad page.
- **Clearing browser data deletes the wallet.** Only the exported seed brings it back.
- **The counter has a gap.** With the seed open in two browsers, or a transaction signed but not yet mined when
  storage is cleared, a one-time key could be used twice, which weakens that key. That's acceptable for test ETH
  and nothing else.
- **It can't rotate.** After 1,008 signatures, start a new wallet.
- **A public deployment needs a faucet and relayer service.** Test ETH and claims use a dev key that ships in the
  page, and anyone could drain it.
- The screen for browsers with no Web Serial wasn't exercised, and Safari and phones haven't been tried.

On the hosted wallet at [qday.unitlabs.xyz](https://qday.unitlabs.xyz), 2026-11-07, after it was redeployed
with the demo: a new demo wallet got test ETH and created its account, paid 0.05 ETH to its own private
address (confirmed in 4.7 seconds), found the payment, and claimed it to a new address, which received
0.0466 ETH. There, the dev key is the hosted devnet's public premine key, so the faucet holds only test ETH.

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

## Public reference network (not our target)

ethrex hegota-testnet: chain id 8141, RPC `https://rpc1.privacy.ethrex.xyz`, explorer
`https://dora.privacy.ethrex.xyz`. Enforces the public `MAX_VERIFY_GAS = 100,000`.
