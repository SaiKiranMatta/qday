#!/usr/bin/env bash
# Deploys a QDay account and sends frame transactions from it, signing with a laptop key or a device.
#
#   scripts/frame-demo.sh deploy
#   scripts/frame-demo.sh send <to> <value_wei>
#   scripts/frame-demo.sh rotate
#   scripts/frame-demo.sh status
#
# Environment:
#   QDAY_RPC_URL          RPC endpoint (default http://127.0.0.1:8545)
#   QDAY_FACTORY          account factory, from tools/devnet/deploy-qday.sh (has a default)
#   QDAY_FUNDING_KEY      key that pays for deploy (needed by deploy)
#   QDAY_FUNDING_WEI      extra wei sent to the account for gas and transfers (default 1 ETH)
#   QDAY_DEVICE_PORT      sign on the device at this port instead of with a laptop key
#   QDAY_STATE_DIR        where state is kept (default ~/.qday-devnet)
#
# A laptop key's seed is kept in the state directory, which is outside the repository.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
RPC=${QDAY_RPC_URL:-http://127.0.0.1:8545}
STATE=${QDAY_STATE_DIR:-$HOME/.qday-devnet}
FACTORY=${QDAY_FACTORY:-0xbCebAd3eCE45625983409B429406a5Dc8d38f876}
FUNDING_WEI=${QDAY_FUNDING_WEI:-1000000000000000000}
PORT=${QDAY_DEVICE_PORT:-}
RECEIPT_TRIES=120
# The mempool gateway can't estimate fees, so plain transactions use a fixed legacy price.
GAS_PRICE=2000000000

die() {
  echo "error: $*" >&2
  exit 1
}

qday() {
  cargo run --quiet --release --manifest-path "$ROOT/Cargo.toml" -p qday-cli -- "$@"
}

# Prints a field from the state directory.
state() {
  [ -f "$STATE/$1" ] || die "no $1 in $STATE: run deploy first"
  cat "$STATE/$1"
}

# Prints the laptop seed, creating one the first time.
seed() {
  if [ ! -f "$STATE/seed" ]; then
    (umask 077 && cast keccak "$(openssl rand -hex 32)" >"$STATE/seed")
  fi
  cat "$STATE/seed"
}

# Prints "<root> <pub_seed>" for the active key.
active_key() {
  if [ -n "$PORT" ]; then
    qday device-pubkey "$PORT" | awk '{print $1, $2}'
  else
    qday pubkey "$(seed)"
  fi
}

deploy() {
  : "${QDAY_FUNDING_KEY:?set QDAY_FUNDING_KEY}"
  read -r root pub_seed < <(active_key)
  account=$(cast call --rpc-url "$RPC" "$FACTORY" 'accountAddress(bytes32,bytes32)(address)' \
    "$root" "$pub_seed")

  cast send --rpc-url "$RPC" --private-key "$QDAY_FUNDING_KEY" --legacy --gas-price "$GAS_PRICE" \
    "$FACTORY" 'createAccount(bytes32,bytes32)' "$root" "$pub_seed" >/dev/null
  cast send --rpc-url "$RPC" --private-key "$QDAY_FUNDING_KEY" --legacy --gas-price "$GAS_PRICE" \
    --value "$FUNDING_WEI" "$account" >/dev/null
  echo "$account" >"$STATE/account"
  echo "account  $account"
  echo "root     $root"
}

next_index() {
  cast call --rpc-url "$RPC" "$1" 'nextIndex()(uint256)' | awk '{print $1}'
}

tree_number() {
  cast call --rpc-url "$RPC" "$1" 'tree()(uint256)' | awk '{print $1}'
}

# Submits a transaction and waits for it to mine.
submit() {
  local hash receipt
  hash=$(cast rpc --rpc-url "$RPC" eth_sendFrameTransaction "$1" | tr -d '"')
  case $hash in 0x*) ;; *) die "the node refused the transaction: $hash" ;; esac
  echo "sent $hash"
  for _ in $(seq "$RECEIPT_TRIES"); do
    receipt=$(cast rpc --rpc-url "$RPC" eth_getTransactionReceipt "$hash")
    if [ "$receipt" != null ]; then
      echo "$receipt" | python3 -c '
import json, sys
r = json.load(sys.stdin)
print("block", int(r["blockNumber"], 16), "gas", int(r["gasUsed"], 16), "status", r["status"])
for i, frame in enumerate(r.get("frameReceipts", [])):
    print("frame", i, frame)
'
      return
    fi
    sleep 1
  done
  die "$hash did not mine"
}

send() {
  [ $# -eq 2 ] || die "usage: send <to> <value_wei>"
  local to=$1 value=$2 account nonce index tree hash signature json
  account=$(state account)
  nonce=$(cast nonce --rpc-url "$RPC" "$account")

  if [ -n "$PORT" ]; then
    json=$(qday device-send "$PORT" "$account" "$nonce" "$to" "$value")
    submit "$json"
    return
  fi
  index=$(next_index "$account")
  tree=$(tree_number "$account")
  hash=$(qday frame-hash "$account" "$nonce" "$index" "$to" "$value")
  signature=$(qday sign "$(seed)" "$index" "$hash" --tree "$tree")
  json=$(qday frame-json "$account" "$nonce" "$index" "$to" "$value" "$signature")
  submit "$json"
}

rotate() {
  local account nonce index tree next_root next_pub_seed message signature json
  account=$(state account)
  nonce=$(cast nonce --rpc-url "$RPC" "$account")

  if [ -n "$PORT" ]; then
    json=$(qday device-rotate "$PORT" "$account" "$nonce")
    submit "$json"
    return
  fi
  index=$(next_index "$account")
  tree=$(tree_number "$account")
  read -r next_root next_pub_seed < <(qday pubkey "$(seed)" --tree "$((tree + 1))")
  message=$(qday frame-rotate-message "$account" "$nonce" "$index" "$next_root" "$next_pub_seed")
  signature=$(qday sign "$(seed)" "$index" "$message" --tree "$tree")
  json=$(qday frame-rotate-json "$account" "$nonce" "$index" "$next_root" "$next_pub_seed" \
    "$signature")
  submit "$json"
}

status() {
  local account
  account=$(state account)
  echo "account   $account"
  echo "balance   $(cast balance --rpc-url "$RPC" "$account")"
  echo "tree      $(tree_number "$account")"
  echo "nextIndex $(next_index "$account")"
  echo "root      $(cast call --rpc-url "$RPC" "$account" 'root()(bytes32)')"
}

mkdir -p "$STATE"
chmod 700 "$STATE"
case ${1:-} in
  deploy) deploy ;;
  send) shift; send "$@" ;;
  rotate) rotate ;;
  status) status ;;
  *) sed -n '2,7p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
