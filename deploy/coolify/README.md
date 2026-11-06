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
| `deployer` | Deploys the XMSS verifier, the account factory and the USDC/WBTC test tokens once blocks flow, then idles. |

Frame transactions work about 7 minutes after the first start, when Hegota activates at epoch 2.

## On Coolify

A Docker Compose application from this repository:

- Compose file: `/deploy/coolify/docker-compose.yml`, base directory `/`.
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
Regenerate it with `deploy/coolify/deployer/export-initcode.sh` (needs forge, cast and solc 0.8.28),
which prints the new addresses for the web app.

The premine key in this stack is public by design. Don't put anything of value on this network.
