#!/usr/bin/env bash
# Creates a QDay account that deploys itself: compute its address, fund it from the faucet, then send its
# first transaction [deploy via factory, verify, spend] signed with a laptop key or the device.
#
#   tools/devnet/create-account.sh                    # laptop key (seed kept in $STATE/seed)
#   QDAY_DEVICE_PORT=/dev/cu.usbmodem1101 tools/devnet/create-account.sh
#
# Environment: QDAY_RPC_URL (node), QDAY_GATEWAY_URL, QDAY_FACTORY, QDAY_STATE_DIR, QDAY_FUNDER_KEY (fund from
# this key instead of the faucet, e.g. the local devnet's premine key). Every network call
# and wait is time-limited, so this never hangs.
set -euo pipefail

RPC=${QDAY_RPC_URL:-https://altmempool.forshtat.com}
GATEWAY=${QDAY_GATEWAY_URL:-https://altmempoolgateway.forshtat.com/rpc}
FACTORY=${QDAY_FACTORY:-0xbCebAd3eCE45625983409B429406a5Dc8d38f876}
STATE=${QDAY_STATE_DIR:-$HOME/.qday-devnet-public}
PORT=${QDAY_DEVICE_PORT:-}
QDAY=${QDAY_BIN:-$HOME/.cargo/shared-target/release/qday}
REQUEST_TIMEOUT=15   # seconds per RPC call
FUND_WAIT=60         # seconds for the faucet payment to land
MINE_WAIT=120        # seconds for the transaction to be mined

die() { echo "error: $*" >&2; exit 1; }
rpc() { # url method params-json
  curl -s --max-time "$REQUEST_TIMEOUT" "$1" -H 'content-type: application/json' \
    --data-binary "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$2\",\"params\":$3}"
}
field() { python3 -c "import json,sys; d=json.load(sys.stdin); print(d.get('result') if 'result' in d else 'ERROR '+json.dumps(d.get('error')))"; }

mkdir -p "$STATE" && chmod 700 "$STATE"
[ -x "$QDAY" ] || die "build the CLI first: cargo build --release -p qday-cli"

if [ -n "$PORT" ]; then
  read -r ROOT PUB_SEED INDEX _ < <("$QDAY" device-pubkey "$PORT")
else
  [ -f "$STATE/seed" ] || (umask 077 && echo "0x$(openssl rand -hex 32)" > "$STATE/seed")
  read -r ROOT PUB_SEED < <("$QDAY" pubkey "$(cat "$STATE/seed")")
  INDEX=0
fi
ACCOUNT=$(cast call --rpc-timeout "$REQUEST_TIMEOUT" "$FACTORY" 'accountAddress(bytes32,bytes32)(address)' "$ROOT" "$PUB_SEED" --rpc-url "$RPC")
echo "account   $ACCOUNT"
echo "key       $ROOT (signature $INDEX)"

CODE=$(rpc "$RPC" eth_getCode "[\"$ACCOUNT\",\"latest\"]" | field)
[ "$CODE" = "0x" ] || die "the account is already deployed"

BALANCE=$(rpc "$RPC" eth_getBalance "[\"$ACCOUNT\",\"latest\"]" | field)
if [ "$BALANCE" = "0x0" ]; then
  if [ -n "${QDAY_FUNDER_KEY:-}" ]; then
    echo "funding   1 ETH from the funder key"
    cast send --rpc-timeout "$REQUEST_TIMEOUT" --quiet --private-key "$QDAY_FUNDER_KEY" --rpc-url "$RPC" \
      --value 1ether "$ACCOUNT" >/dev/null || die "funding failed"
  else
    echo "funding   from the faucet"
    rpc "$GATEWAY" skandha_explorer_faucet "[\"$ACCOUNT\"]" >/dev/null || true
  fi
  for _ in $(seq 1 $((FUND_WAIT / 3))); do
    BALANCE=$(rpc "$RPC" eth_getBalance "[\"$ACCOUNT\",\"latest\"]" | field)
    [ "$BALANCE" != "0x0" ] && break
    sleep 3
  done
  [ "$BALANCE" != "0x0" ] || die "the faucet didn't pay within ${FUND_WAIT}s"
fi
echo "balance   $(cast from-wei "$(cast to-dec "$BALANCE")") ETH"

if [ -n "$PORT" ]; then
  echo "signing   on the device: approve 'Create account' there"
  JSON=$("$QDAY" device-create "$PORT" "$ACCOUNT" "$FACTORY")
else
  HASH=$("$QDAY" frame-create-hash "$ACCOUNT" "$INDEX" "$FACTORY" "$ROOT" "$PUB_SEED")
  SIG=$("$QDAY" sign "$(cat "$STATE/seed")" "$INDEX" "$HASH")
  JSON=$("$QDAY" frame-create-json "$ACCOUNT" "$INDEX" "$FACTORY" "$ROOT" "$PUB_SEED" "$SIG")
fi

SENT=$(rpc "$GATEWAY" eth_sendFrameTransaction "[$JSON]" | field)
case $SENT in 0x*) echo "sent      $SENT" ;; *) die "the gateway refused it: $SENT" ;; esac
echo "$ACCOUNT" > "$STATE/account"

for i in $(seq 1 $((MINE_WAIT / 3))); do
  RECEIPT=$(rpc "$RPC" eth_getTransactionReceipt "[\"$SENT\"]")
  if echo "$RECEIPT" | python3 -c 'import json,sys; sys.exit(0 if json.load(sys.stdin).get("result") else 1)'; then
    echo "$RECEIPT" | python3 -c '
import json, sys
r = json.load(sys.stdin)["result"]
print("mined     block", int(r["blockNumber"], 16), "status", r["status"], "gas", int(r["gasUsed"], 16))
for i, f in enumerate(r.get("frameReceipts", [])):
    print("  frame", i, "status", f["status"], "gas", int(f["gasUsed"], 16), "state gas", int(f.get("stateGasUsed", "0x0"), 16))
'
    echo "code      $(rpc "$RPC" eth_getCode "[\"$ACCOUNT\",\"latest\"]" | field | wc -c | awk '{print ($1 - 3) / 2}') bytes"
    exit 0
  fi
  sleep 3
done
STATUS=$(rpc "$GATEWAY" skandha_explorer_getTransaction "[\"$SENT\"]" | python3 -c 'import json,sys; e=(json.load(sys.stdin).get("result") or {}).get("entry") or {}; print(e.get("status","unknown"), e.get("revertReason",""))')
die "not mined within ${MINE_WAIT}s; the gateway says: $STATUS"
