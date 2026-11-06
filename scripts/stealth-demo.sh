#!/usr/bin/env bash
# Makes stealth payments through the lattice pool, signing with a laptop key or a device. The signer
# is an unreviewed variant, so this is for test networks only.
#
#   scripts/stealth-demo.sh deploy
#   scripts/stealth-demo.sh address
#   scripts/stealth-demo.sh pay <value_wei>
#   scripts/stealth-demo.sh scan
#   scripts/stealth-demo.sh spend <key_hash> <to> <fee_wei>
#
# Environment:
#   QDAY_RPC_URL      RPC endpoint (default http://127.0.0.1:8545)
#   QDAY_FUNDING_KEY  key that deploys, pays and relays
#   QDAY_DEVICE_PORT  use the device at this port instead of a laptop key
#   QDAY_STATE_DIR    where state is kept (default ~/.qday-devnet/stealth)
#
# A laptop key's seed is kept in the state directory, which is outside the repository.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
RPC=${QDAY_RPC_URL:-http://127.0.0.1:8545}
STATE=${QDAY_STATE_DIR:-$HOME/.qday-devnet/stealth}
PORT=${QDAY_DEVICE_PORT:-}
# The mempool gateway can't estimate fees, so plain transactions use a fixed legacy price.
GAS_PRICE=2000000000
DEPLOY_GAS=40000000
SPEND_GAS=3000000

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

# Sends a transaction, checks it worked, prints its gas to stderr and its receipt to stdout.
transact() {
  local name=$1 receipt
  shift
  : "${QDAY_FUNDING_KEY:?set QDAY_FUNDING_KEY}"
  receipt=$(cast send --rpc-url "$RPC" --private-key "$QDAY_FUNDING_KEY" --legacy \
    --gas-price "$GAS_PRICE" --json "$@")
  echo "$receipt" | python3 -c "
import json, sys
r = json.load(sys.stdin)
print('$name', 'gas', int(r['gasUsed'], 16), 'status', r['status'], file=sys.stderr)
sys.exit(0 if r['status'] == '0x1' else 1)"
  echo "$receipt"
}

contract_address() {
  python3 -c "import json, sys; print(json.load(sys.stdin)['contractAddress'])"
}

# Deploys raw bytes as a contract's code, and prints its address.
deploy_raw() {
  local name=$1 runtime=${2#0x} size
  size=$(printf '%04x' $((${#runtime} / 2)))
  transact "$name" --gas-limit "$DEPLOY_GAS" \
    --create "0x61${size}600e60003961${size}6000f3${runtime}" | contract_address
}

create() {
  local project=$1
  shift
  forge create --root "$ROOT/$project" --rpc-url "$RPC" --private-key "$QDAY_FUNDING_KEY" \
    --legacy --gas-price "$GAS_PRICE" --broadcast "$@" | awk '/Deployed to:/ {print $3}'
}

deploy() {
  : "${QDAY_FUNDING_KEY:?set QDAY_FUNDING_KEY}"
  local helper verifier matrix pool
  helper=$(deploy_raw helper "$(tr -d '\n' <"$ROOT/contracts/mldsa/helpers/f1600_170.hex")")
  matrix=$(deploy_raw matrix "$(qday stealth-matrix)")
  verifier=$(create contracts/mldsa src/MLDSA44InlineVerifier.sol:MLDSA44InlineVerifier \
    --constructor-args "$helper")
  [ -n "$verifier" ] || die "the verifier did not deploy"
  pool=$(create contracts src/QDayLatticePool.sol:QDayLatticePool \
    --constructor-args "$verifier" "$matrix")
  [ -n "$pool" ] || die "the pool did not deploy"
  echo "$pool" >"$STATE/pool"
  echo "matrix   $matrix"
  echo "verifier $verifier"
  echo "pool     $pool"
}

address() {
  local meta view
  if [ -n "$PORT" ]; then
    meta=$(qday device-stealth-address "$PORT")
    view=$(qday device-view-key "$PORT")
  else
    meta=$(qday stealth-address "$(seed)")
    view=$(qday stealth-view "$(seed)")
  fi
  echo "$meta" >"$STATE/address"
  (umask 077 && echo "$view" >"$STATE/view")
  echo "address ${meta:0:34}... ($(((${#meta} - 2) / 2)) bytes)"
}

pay() {
  [ $# -eq 1 ] || die "usage: pay <value_wei>"
  local key_hash tag ciphertext
  read -r key_hash tag ciphertext < <(qday stealth-pay "$(state address address)" \
    "0x$(openssl rand -hex 64)")
  transact pay --value "$1" "$(state pool)" 'pay(bytes32,bytes1,bytes)' "$key_hash" \
    "$(printf '0x%02x' "$tag")" "$ciphertext" >/dev/null
  echo "key hash $key_hash"
}

scan() {
  local logs=$STATE/announcements
  cast logs --rpc-url "$RPC" --json --from-block 0 --to-block latest --address "$(state pool)" \
    'Announcement(bytes32,bytes1,bytes)' | python3 -c '
import json, sys
for log in json.load(sys.stdin):
    data = bytes.fromhex(log["data"][2:])
    length = int.from_bytes(data[64:96], "big")
    print(log["topics"][1], data[0], "0x" + data[96:96 + length].hex())
' >"$logs"
  qday stealth-scan "$(state view address)" "$(state address address)" "$logs" |
    tee "$STATE/found" | cut -d' ' -f1
  echo "scanned $(wc -l <"$logs" | tr -d ' ') announcements, found $(grep -c 0x "$STATE/found" || true)" >&2
}

spend() {
  [ $# -eq 3 ] || die "usage: spend <key_hash> <to> <fee_wei>"
  local key_hash=$1 to=$2 fee=$3 pool ciphertext key signature
  pool=$(state pool)
  ciphertext=$(awk -v hash="$key_hash" '$1 == hash {print $2}' "$(printf '%s/found' "$STATE")")
  [ -n "$ciphertext" ] || die "$key_hash isn't in what scan found"
  key=$(qday stealth-key "$(state view address)" "$(state address address)" "$ciphertext")
  if [ -n "$PORT" ]; then
    signature=$(qday device-stealth-spend "$PORT" "$ciphertext" "$key_hash" "$pool" "$to" "$fee")
  else
    signature=$(qday stealth-sign "$(seed)" "$ciphertext" "$key_hash" "$pool" "$to" "$fee" \
      "0x$(openssl rand -hex 32)")
  fi
  transact spend --gas-limit "$SPEND_GAS" "$pool" 'spend(bytes,bytes,address,uint256)' "$key" \
    "$signature" "$to" "$fee" >/dev/null
}

mkdir -p "$STATE"
chmod 700 "$STATE"
case ${1:-} in
  deploy) deploy ;;
  address) address ;;
  pay) shift; pay "$@" ;;
  scan) scan ;;
  spend) shift; spend "$@" ;;
  *) sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'; exit 2 ;;
esac
