#!/usr/bin/env bash
# Deploys an ML-DSA account and sends frame transactions from it, signing with a laptop key or a
# device.
#
#   scripts/mldsa-demo.sh deploy
#   scripts/mldsa-demo.sh send <to> <value_wei>
#   scripts/mldsa-demo.sh status
#
# Environment:
#   QDAY_RPC_URL          RPC endpoint (default http://127.0.0.1:8545)
#   QDAY_STAKING_REGISTRY staking registry address, or empty for no stake
#   QDAY_STAKE            stake in wei (default 12000000000000000, which is 0.012 ETH)
#   QDAY_UNSTAKE_DELAY    unstake delay in seconds (default 86400)
#   QDAY_FUNDING_KEY      key that pays for deploy (needed by deploy)
#   QDAY_FUNDING_WEI      wei sent to the account for gas and transfers (default 1 ETH)
#   QDAY_DEVICE_PORT      sign on the device at this port instead of with a laptop key
#   QDAY_STATE_DIR        where state is kept (default ~/.qday-devnet/mldsa)
#
# A laptop key's seed is kept in the state directory, which is outside the repository.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
RPC=${QDAY_RPC_URL:-http://127.0.0.1:8545}
STATE=${QDAY_STATE_DIR:-$HOME/.qday-devnet/mldsa}
REGISTRY=${QDAY_STAKING_REGISTRY:-}
STAKE=${QDAY_STAKE:-12000000000000000}
UNSTAKE_DELAY=${QDAY_UNSTAKE_DELAY:-86400}
FUNDING_WEI=${QDAY_FUNDING_WEI:-1000000000000000000}
PORT=${QDAY_DEVICE_PORT:-}
ZERO_ADDRESS=0x0000000000000000000000000000000000000000
ACCOUNT_SALT=0x0000000000000000000000000000000000000000000000000000000000000000
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

state() {
  [ -f "$STATE/$1" ] || die "no $1 in $STATE: run deploy first"
  cat "$STATE/$1"
}

seed() {
  if [ ! -f "$STATE/seed" ]; then
    (umask 077 && cast keccak "$(openssl rand -hex 32)" >"$STATE/seed")
  fi
  cat "$STATE/seed"
}

public_key() {
  if [ -n "$PORT" ]; then
    qday device-mlpubkey "$PORT"
  else
    qday mldsa-pubkey "$(seed)"
  fi
}

# Reads a field from a JSON receipt on stdin.
field() {
  python3 -c "import json,sys; r=json.load(sys.stdin); v=r['$1']; print(int(v,16) if '$1'=='gasUsed' else v)"
}

# Sends a transaction, checks it worked, prints its gas to stderr and its receipt to stdout.
transact() {
  local name=$1 receipt
  shift
  receipt=$(cast send --rpc-url "$RPC" --private-key "$QDAY_FUNDING_KEY" --legacy \
    --gas-price "$GAS_PRICE" --json "$@")
  [ "$(echo "$receipt" | field status)" = 0x1 ] || die "$name failed"
  echo "$name gas $(echo "$receipt" | field gasUsed)" >&2
  echo "$receipt"
}

# Creates a contract with forge and prints its address.
create() {
  local project=$1
  shift
  forge create --root "$ROOT/$project" --rpc-url "$RPC" --private-key "$QDAY_FUNDING_KEY" \
    --legacy --gas-price "$GAS_PRICE" --broadcast "$@" | awk '/Deployed to:/ {print $3}'
}

# Deploys the verifier and its helper once. Every account on the chain can share them.
deploy_shared() {
  if [ -f "$STATE/verifier" ] && [ -f "$STATE/factory" ]; then
    return
  fi
  local runtime size initcode helper verifier factory
  runtime=$(tr -d '\n' <"$ROOT/contracts/mldsa/helpers/f1600_170.hex")
  size=$(printf '%04x' $((${#runtime} / 2)))
  initcode="0x61${size}8061000b5f395ff3${runtime}"
  helper=$(transact helper --create "$initcode" | field contractAddress)
  verifier=$(create contracts/mldsa src/MLDSA44Verifier.sol:MLDSA44Verifier \
    --constructor-args "$helper")
  [ -n "$verifier" ] || die "the verifier did not deploy"
  factory=$(create contracts src/QDayMlDsaFactory.sol:QDayMlDsaFactory)
  [ -n "$factory" ] || die "the factory did not deploy"
  echo "$helper" >"$STATE/helper"
  echo "$verifier" >"$STATE/verifier"
  echo "$factory" >"$STATE/factory"
}

deploy() {
  : "${QDAY_FUNDING_KEY:?set QDAY_FUNDING_KEY}"
  deploy_shared
  local verifier factory pk blob blob_at deployed verifier_hash blob_hash registry account

  verifier=$(state verifier)
  factory=$(state factory)
  pk=$(public_key)
  # This refuses a key of the wrong length, and a degenerate key.
  blob=$(qday mldsa-blob "$pk")

  blob_at=$(cast call --rpc-url "$RPC" "$factory" 'predictBlob(bytes)(address)' "$blob")
  transact 'key blob' "$factory" 'deployBlob(bytes)' "$blob" >/dev/null
  deployed=$(cast code --rpc-url "$RPC" "$blob_at")
  [ "$deployed" = "$blob" ] || die "the deployed blob isn't byte for byte prepare(pk)"

  verifier_hash=$(cast keccak "$(cast code --rpc-url "$RPC" "$verifier")")
  blob_hash=$(cast keccak "$blob")
  registry=${REGISTRY:-$ZERO_ADDRESS}
  local stake=$STAKE
  [ "$registry" != "$ZERO_ADDRESS" ] || stake=0

  local settings=("$verifier" "$verifier_hash" "$blob_at" "$blob_hash" "$registry" "$stake"
    "$UNSTAKE_DELAY" "$ACCOUNT_SALT")
  local signature='(address,bytes32,address,bytes32,address,uint256,uint32,bytes32)'
  account=$(cast call --rpc-url "$RPC" "$factory" "predictAccount$signature(address)" \
    "${settings[@]}")
  transact account --value "$stake" "$factory" "deployAccount$signature" "${settings[@]}" \
    >/dev/null
  transact funding --value "$FUNDING_WEI" "$account" >/dev/null

  echo "$blob_at" >"$STATE/blob"
  echo "$account" >"$STATE/account"
  echo "verifier $verifier"
  echo "blob     $blob_at"
  echo "account  $account"
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
    print("frame", i, "gas", int(frame["gasUsed"], 16), "status", frame["status"])
'
      return
    fi
    sleep 1
  done
  die "$hash did not mine"
}

send() {
  [ $# -eq 2 ] || die "usage: send <to> <value_wei>"
  local to=$1 value=$2 account nonce hash signature json
  account=$(state account)
  nonce=$(cast nonce --rpc-url "$RPC" "$account")

  if [ -n "$PORT" ]; then
    json=$(qday device-ml-send "$PORT" "$account" "$nonce" "$to" "$value")
    submit "$json"
    return
  fi
  hash=$(qday ml-frame-hash "$account" "$nonce" "$to" "$value")
  signature=$(qday mldsa-sign "$(seed)" "$hash" "0x$(openssl rand -hex 32)")
  json=$(qday ml-frame-json "$account" "$nonce" "$to" "$value" "$signature")
  submit "$json"
}

status() {
  local account
  account=$(state account)
  echo "account $account"
  echo "balance $(cast balance --rpc-url "$RPC" "$account")"
  echo "nonce   $(cast nonce --rpc-url "$RPC" "$account")"
  echo "blob    $(state blob)"
}

mkdir -p "$STATE"
chmod 700 "$STATE"
case ${1:-} in
  deploy) deploy ;;
  send) shift; send "$@" ;;
  status) status ;;
  *) sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
