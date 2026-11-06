# Hosted devnet (Coolify)

The same EIP-8141 devnet as `tools/devnet/local/up.sh`, as one self-contained Compose stack that a
server can keep running: chain 1337, 6-second slots, the ERC-8421 gateway with QDay's verify budget,
and QDay's contracts at their usual addresses.

| Service | What it does |
|---|---|
| `genesis`, `validator-keys` | Write the chain's genesis and the 32 validators' keystores on the first start, then idle. Later starts keep the chain. |
| `ethrex` | The execution node. Its RPC (port 8545) is one of the two public endpoints. |
| `lighthouse-beacon`, `lighthouse-beacon-2`, `lighthouse-validator` | Consensus. The second beacon only exists because Lighthouse won't build blocks with no peers. |
| `gateway` | Accepts frame transactions (port 14337), the other public endpoint. |
| `deployer` | Deploys QDay's contracts once blocks flow (below), then idles. |

Frame transactions work about 7 minutes after the first start, when Hegota activates at epoch 2.

The deployer puts every contract at the address the web app expects, through the CREATE2 factory:

| Contract | Address | For |
|---|---|---|
| `XMSSVerifier` | `0xe0Ab78755c24867147473b12ffA430D24C1ed127` | XMSS accounts |
| `QDayAccountFactory` | `0xbCebAd3eCE45625983409B429406a5Dc8d38f876` | XMSS accounts |
| Test USDC, test WBTC | `0xd1108cBf3C5E7b44D7A58d4b2cdA492a0DDF6a72`, `0xb212346871a9a08494e165F5AeFe655f06A5B75F` | Token transfers |
| Keccak helper | `0x6aF3BE51e700B26C500490D12Adb587575D9Aa3c` | Both ML-DSA verifiers |
| `MLDSA44Verifier` | `0x20a8128F20FAF48A9A26Bb83c332582520100954` | ML-DSA accounts |
| `QDayMlDsaFactory` | `0x6aD8F805f02509bdf646406b9108dd3070E4174d` | ML-DSA accounts |
| `NaiveStakingRegistry` | `0xb9654f4208EE0809D212B66eD145A3CC07464857` | ML-DSA accounts stake here |
| `MLDSA44InlineVerifier` | `0x9bCbE9018aEbFA0c40cDd88287BC25d8A1A0003c` | Private payments |
| Shared matrix | `0x70ca32663F21540DF18892d64d864120c46EfD0A` | Private payments |
| `QDayLatticePool` | `0x4eBfa044FA7a52CfDE7Fc1B6d8Fb3b4b0669f354` | Private payments |

The ML-DSA contracts cost up to about 34M gas each to deploy, because code costs about 1,555 gas per
byte on this chain, so the first start takes a few more blocks. The registry is the sidecar's
`NaiveStakingRegistry` (MIT). This gateway doesn't read stakes: its one verify budget, raised to
10,000,000 gas, covers an ML-DSA check (about 1.25M) as well as an XMSS one.

## Live

QDay's own instance runs on Coolify in the `qday` project:

| Endpoint | URL |
|---|---|
| Node RPC | `https://rpc-qday.unitlabs.xyz` |
| Gateway | `https://gateway-qday.unitlabs.xyz/rpc` |

Run the web app against it with `VITE_QDAY_NETWORK=hosted pnpm dev`.

## On Coolify

A Docker Compose application from this repository:

- Base directory `/deploy/coolify`, compose file `/docker-compose.yml`. Coolify resolves the build
  folders (`./genesis`, `./gateway`, `./deployer`) from the base directory, so it must be this folder.
- Domains: `ethrex` on port 8545 (the RPC) and `gateway` on port 14337.
- The server must be x86-64: the node and gateway images are published for it only.

Redeploying keeps the chain, because it lives in the stack's volumes. To start a new chain, stop the
application, delete its volumes, and deploy again; the contracts are redeployed at the same addresses.

## Anywhere else

```sh
docker compose -f deploy/coolify/docker-compose.yml up -d --build
```

Nothing is published to the host by default; add `ports` for `ethrex` and `gateway` to reach them.

## After changing a contract

The deployer uses pre-built init code, so the addresses can't drift with a compiler update.
Regenerate it with `deploy/coolify/deployer/export-initcode.sh` (needs forge, cast, cargo, solc 0.8.28
and the sidecar repository that `tools/devnet/local/up.sh` clones), which prints the new addresses
for the web app.

The premine key in this stack is public by design. Don't put anything of value on this network.
