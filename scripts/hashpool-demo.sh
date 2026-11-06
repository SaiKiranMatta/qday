#!/usr/bin/env bash
# Pays fresh one-time keys through the hash pool, signing with a laptop key or a device.
#
#   scripts/hashpool-demo.sh deploy
#   scripts/hashpool-demo.sh invoice <number>
#   scripts/hashpool-demo.sh pay <number> <value_wei>
#   scripts/hashpool-demo.sh spend <number> <to> <fee_wei>
#   scripts/hashpool-demo.sh status <number>
#
# Environment:
#   QDAY_RPC_URL      RPC endpoint (default http://127.0.0.1:8545)
#   QDAY_FUNDING_KEY  key that deploys, pays and relays
#   QDAY_DEVICE_PORT  sign on the device at this port instead of with a laptop key
#   QDAY_STATE_DIR    where state is kept (default ~/.qday-devnet/hashpool)
#
# A laptop key's seed is kept in the state directory, which is outside the repository.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
RPC=${QDAY_RPC_URL:-http://127.0.0.1:8545}
STATE=${QDAY_STATE_DIR:-$HOME/.qday-devnet/hashpool}
PORT=${QDAY_DEVICE_PORT:-}
# The mempool gateway can't estimate fees, so plain transactions use a fixed legacy price.
GAS_PRICE=2000000000
SPEND_GAS=1000000

die() {
  echo "error: $*" >&2
  exit 1
}

qday() {
  cargo run --quiet --release --manifest-path "$ROOT/Cargo.toml" -p qday-cli -- "$@"
}

state() {
  [ -f "$STATE/$1" ] || die "no $1 in $STATE: run ${2:-deploy} first"
  cat "$STATE/$1"
}

seed() {
  if [ ! -f "$STATE/seed" ]; then
    (umask 077 && cast keccak "$(openssl rand -hex 32)" >"$STATE/seed")
  fi
  cat "$STATE/seed"
}

# Sends a transaction, checks it worked, and prints its gas.
transact() {
  local name=$1 receipt
  shift
  : "${QDAY_FUNDING_KEY:?set QDAY_FUNDING_KEY}"
  receipt=$(cast send --rpc-url "$RPC" --private-key "$QDAY_FUNDING_KEY" --legacy \
    --gas-price "$GAS_PRICE" --json "$@")
  echo "$receipt" | python3 -c "
import json, sys
r = json.load(sys.stdin)
print('$name', 'gas', int(r['gasUsed'], 16), 'status', r['status'])
sys.exit(0 if r['status'] == '0x1' else 1)"
}

deploy() {
  : "${QDAY_FUNDING_KEY:?set QDAY_FUNDING_KEY}"
  local pool
  pool=$(forge create --root "$ROOT/contracts" --rpc-url "$RPC" --private-key "$QDAY_FUNDING_KEY" \
    --legacy --gas-price "$GAS_PRICE" --broadcast src/QDayHashPool.sol:QDayHashPool |
    awk '/Deployed to:/ {print $3}')
  [ -n "$pool" ] || die "the pool did not deploy"
  echo "$pool" >"$STATE/pool"
  echo "pool $pool"
}

invoice() {
  [ $# -eq 1 ] || die "usage: invoice <number>"
  local key
  if [ -n "$PORT" ]; then
    key=$(qday device-invoice "$PORT" "$1")
  else
    key=$(qday invoice-key "$(seed)" "$1")
  fi
  echo "$key" >"$STATE/invoice-$1"
  echo "$key"
}

pay() {
  [ $# -eq 2 ] || die "usage: pay <number> <value_wei>"
  local leaf
  read -r leaf _ < <(state "invoice-$1" invoice)
  transact pay --value "$2" "$(state pool)" 'pay(bytes32)' "$leaf"
}

spend() {
  [ $# -eq 3 ] || die "usage: spend <number> <to> <fee_wei>"
  local number=$1 to=$2 fee=$3 pool leaf pub_seed signature
  pool=$(state pool)
  read -r leaf pub_seed < <(state "invoice-$number" invoice)
  if [ -n "$PORT" ]; then
    signature=$(qday device-invoice-spend "$PORT" "$number" "$pool" "$to" "$fee")
  else
    signature=$(qday invoice-sign "$(seed)" "$number" "$pool" "$to" "$fee")
  fi
  transact spend --gas-limit "$SPEND_GAS" "$pool" \
    'spend(bytes32,bytes32,bytes,address,uint256)' "$leaf" "$pub_seed" "$signature" "$to" "$fee"
}

status() {
  [ $# -eq 1 ] || die "usage: status <number>"
  local leaf
  read -r leaf _ < <(state "invoice-$1" invoice)
  echo "note $(cast call --rpc-url "$RPC" "$(state pool)" 'notes(bytes32)(uint256)' "$leaf")"
}

mkdir -p "$STATE"
chmod 700 "$STATE"
case ${1:-} in
  deploy) deploy ;;
  invoice) shift; invoice "$@" ;;
  pay) shift; pay "$@" ;;
  spend) shift; spend "$@" ;;
  status) shift; status "$@" ;;
  *) sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
