#!/usr/bin/env bash
# Starts a private EIP-8141 devnet on this machine: ethrex + Lighthouse (from the mempool-workshop-sidecar
# repo's Compose stack) and the ERC-8421 sidecar as the local gateway.
#
#   tools/devnet/local/up.sh        # start; frame transactions work about 7 minutes after genesis
#   tools/devnet/local/down.sh      # stop and wipe
#
# Endpoints when up: node http://127.0.0.1:8545, gateway http://127.0.0.1:14337/rpc, chain 1337.
# Funded key: the devnet's throwaway premine key 0x06f9…ea15 (address 0x5544…16f9), public by design.
# Every step has a time limit, so a stuck pull or container fails loudly instead of hanging.
set -euo pipefail
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)

SIDECAR_REPO=${SIDECAR_REPO:-$HOME/Desktop/eth-mumbai/mempool-workshop-sidecar}
WORK=${QDAY_LOCAL_DEVNET:-$HOME/.qday-local-devnet}
GENESIS_DELAY=${GENESIS_DELAY:-60}
SIDECAR_IMAGE=ghcr.io/forshtat/mempool-workshop-sidecar:latest
PREMINE_KEY=0x06f911c1ecb2053bb443ba0a0e28d630c583c3c0c2f334a9ecdc8c7c6ad4ea15

PULL_TIMEOUT=900     # first pulls are a few GB
STEP_TIMEOUT=180     # any single compose step
BEACON_WAIT=120      # beacon HTTP API coming up
NODE_WAIT=60         # ethrex RPC answering

# Runs a command with a time limit (macOS has no `timeout`).
limit() { local secs=$1; shift; perl -e 'alarm shift; exec @ARGV or die "exec: $!"' "$secs" "$@"; }
die() { echo "error: $*" >&2; exit 1; }

[ -d "$SIDECAR_REPO/deploy/devnet" ] || limit 120 git clone -q --depth 1 https://github.com/forshtat/mempool-workshop-sidecar.git "$SIDECAR_REPO"

# Create every bind-mounted folder first: Docker Desktop can fail to create them itself right after a wipe.
mkdir -p "$WORK"/data/{genesis,validator-keys,ethrex,lighthouse-beacon,lighthouse-beacon-2,lighthouse-validator} "$WORK/sidecar-data"
# The ethrex and sidecar images are amd64 only; Docker Desktop runs them under emulation on Apple Silicon.
sed 's|^    image: ghcr.io/forshtat/ethrex:eip8141-tracer-latest|    image: ghcr.io/forshtat/ethrex:eip8141-tracer-latest\n    platform: linux/amd64|' \
  "$SIDECAR_REPO/deploy/devnet/docker-compose.yml" > "$WORK/docker-compose.yml"
sed "s/^export GENESIS_DELAY=.*/export GENESIS_DELAY=\"$GENESIS_DELAY\"/" \
  "$SIDECAR_REPO/deploy/devnet/genesis.values.env" > "$WORK/genesis.values.env"
cd "$WORK"
export LIGHTHOUSE_BEACON_1_IP=pending LIGHTHOUSE_BEACON_1_ENR=pending

echo "== pulling images (up to ${PULL_TIMEOUT}s)"
limit "$PULL_TIMEOUT" docker compose pull --quiet || die "image pull timed out or failed"
limit "$PULL_TIMEOUT" docker pull --quiet --platform linux/amd64 "$SIDECAR_IMAGE" >/dev/null || die "sidecar pull failed"

echo "== genesis"
export GENESIS_TIMESTAMP=$(date +%s)
limit "$STEP_TIMEOUT" docker compose up genesis validator-keys >/dev/null || die "genesis timed out or failed"
limit "$STEP_TIMEOUT" docker compose up -d --no-deps ethrex || die "ethrex didn't start"

echo "== consensus"
# The beacon must advertise its own container IP, which Docker only assigns at start. On the Compose
# network ethrex takes .2, so the beacon, started next, takes .3 (the upstream README relies on the same).
SUBNET_BASE=$(docker network inspect eip8141-devnet_default --format '{{(index .IPAM.Config 0).Subnet}}' | cut -d/ -f1 | cut -d. -f1-3)
export LIGHTHOUSE_BEACON_1_IP="$SUBNET_BASE.3"
limit "$STEP_TIMEOUT" docker compose up -d --no-deps lighthouse-beacon || die "beacon didn't start"
ACTUAL_IP=$(docker inspect eip8141-devnet-lighthouse-beacon-1 \
  --format '{{range $n, $v := .NetworkSettings.Networks}}{{$v.IPAddress}}{{end}}')
[ "$ACTUAL_IP" = "$LIGHTHOUSE_BEACON_1_IP" ] || die "the beacon got $ACTUAL_IP, not $LIGHTHOUSE_BEACON_1_IP; run down.sh and retry"
ENR=""
for _ in $(seq 1 $((BEACON_WAIT / 3))); do
  # The Lighthouse image has no wget or curl, so ask from a throwaway curl container on the same network.
  ENR=$(limit 20 docker run --rm --network eip8141-devnet_default curlimages/curl:8.11.1 -s --max-time 5 \
      http://lighthouse-beacon:5052/eth/v1/node/identity 2>/dev/null \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["enr"])' 2>/dev/null || true)
  [ -n "$ENR" ] && break
  sleep 3
done
[ -n "$ENR" ] || die "the beacon API didn't answer within ${BEACON_WAIT}s"
export LIGHTHOUSE_BEACON_1_ENR=$ENR
printf 'LIGHTHOUSE_BEACON_1_IP=%s\nLIGHTHOUSE_BEACON_1_ENR=%s\n' "$LIGHTHOUSE_BEACON_1_IP" "$LIGHTHOUSE_BEACON_1_ENR" > .env
limit "$STEP_TIMEOUT" docker compose up -d --no-deps lighthouse-beacon-2 lighthouse-validator || die "peer/validator didn't start"

echo "== gateway"
cat > sidecar-config.json <<EOF
{
  "entryPoints": ["0x00000000000000000000000000000000000000aa"],
  "canonicalEntryPoint": "0x00000000000000000000000000000000000000aa",
  "standardMempoolId": "QmcFZKUX9qoo3StwVycJTAhwdiqbEjcwNvQVK246p3z1rk",
  "supportedMempools": [
    { "mempoolId": "QmcFZKUX9qoo3StwVycJTAhwdiqbEjcwNvQVK246p3z1rk",
      "entryPoint": "0x00000000000000000000000000000000000000aa" }
  ],
  "relayers": ["$PREMINE_KEY"],
  "rpcEndpoint": "http://ethrex:8545",
  "rpcEndpointSubmit": "http://ethrex:8545"
}
EOF
# The published sidecar hard-codes the alt mempool's verify budget at 100,000 gas, too little for an
# XMSS check. Patch that one compiled line to read MAX_VERIFY_GAS, and mount it over the original.
PREFIX_JS=/usr/app/packages/executor/lib/services/UserOpValidation/utils/validationPrefix.js
docker rm -f qday-sidecar-src >/dev/null 2>&1 || true
docker create --platform linux/amd64 --name qday-sidecar-src "$SIDECAR_IMAGE" >/dev/null
docker cp "qday-sidecar-src:$PREFIX_JS" "$WORK/validationPrefix.js" && docker rm -f qday-sidecar-src >/dev/null
sed -i '' 's/^export const MAX_VERIFY_GAS = BigInt(100000);/export const MAX_VERIFY_GAS = BigInt(process.env.MAX_VERIFY_GAS ?? 100000);/' "$WORK/validationPrefix.js"
grep -q 'process.env.MAX_VERIFY_GAS' "$WORK/validationPrefix.js" || die "couldn't patch the verify budget; the sidecar image changed"
docker rm -f qday-local-gateway >/dev/null 2>&1 || true
limit "$STEP_TIMEOUT" docker run -d --platform linux/amd64 --name qday-local-gateway --network eip8141-devnet_default \
  -p 14337:14337 -e MAX_VERIFY_GAS="${MAX_VERIFY_GAS:-10000000}" \
  --mount "type=bind,source=$WORK/validationPrefix.js,target=$PREFIX_JS,readonly" \
  --mount "type=bind,source=$WORK/sidecar-config.json,target=/usr/app/config.json,readonly" \
  --mount "type=bind,source=$WORK/sidecar-data,target=/usr/app/data" \
  "$SIDECAR_IMAGE" standalone --configFile /usr/app/config.json --dataDir /usr/app/data \
  --api.address 0.0.0.0 --api.port 14337 --api.wsPort 14337 \
  --executor.bundlingMode auto --redirectRpc --explorer.enable >/dev/null || die "gateway didn't start"

echo "== checking the node"
for _ in $(seq 1 $((NODE_WAIT / 3))); do
  curl -s --max-time 3 http://127.0.0.1:8545 -H 'content-type: application/json' \
    -d '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' | grep -q result && break
  sleep 3
done
docker ps --format '  {{.Names}}\t{{.Status}}' | grep -E "eip8141|qday" || true
echo "== deploying the QDay verifier and factory"
QDAY_RPC_URL=http://127.0.0.1:8545 QDAY_DEPLOY_KEY=$PREMINE_KEY limit 300 "$SCRIPT_DIR/../deploy-qday.sh" || echo "  (deploy later: frame contracts need the chain running)"
echo "== up. genesis at $(date -r $((GENESIS_TIMESTAMP + GENESIS_DELAY)) +%T); frame transactions from about $(date -r $((GENESIS_TIMESTAMP + GENESIS_DELAY + 384)) +%T)"
