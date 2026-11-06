#!/usr/bin/env bash
# Deploys the XMSS verifier and the QDay account factory through the CREATE2 factory, so both land
# at the same addresses after every devnet reset. Safe to rerun: contracts already there are skipped.
# Needs: forge, cast, and ../../.env (ask Sainath for the team key), or QDAY_RPC_URL and QDAY_DEPLOY_KEY,
# e.g. for the local devnet: QDAY_RPC_URL=http://127.0.0.1:8545 QDAY_DEPLOY_KEY=<premine key>.
set -euo pipefail
cd "$(dirname "$0")"
if [ -z "${QDAY_RPC_URL:-}" ] || [ -z "${QDAY_DEPLOY_KEY:-}" ]; then set -a; source ../../.env; set +a; fi

CONTRACTS=../../contracts
CREATE2=0x4e59b44847b379578588920cA78FbF26c0B4956C
SALT=$(cast keccak "qday-devnet-v2")
RPC=${QDAY_RPC_URL:-$DEVNET_RPC_URL}
KEY=${QDAY_DEPLOY_KEY:-$TEAM_PRIVATE_KEY}
export ETH_RPC_TIMEOUT=30   # seconds per RPC call

(cd "$CONTRACTS" && forge build --quiet)
bytecode() { python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['bytecode']['object'])" "$CONTRACTS/out/$1.sol/$1.json"; }

# Deploys init code through CREATE2 unless something already lives at its address.
deploy() {
  local init=$1 addr
  addr=$(cast to-check-sum-address "0x$(cast keccak "0xff${CREATE2#0x}${SALT#0x}$(cast keccak "$init" | cut -c3-)" | cut -c27-)")
  if [ "$(cast codesize "$addr" --rpc-url "$RPC")" = "0" ]; then
    cast send --quiet --private-key "$KEY" --rpc-url "$RPC" \
      "$CREATE2" "${SALT}${init#0x}" >/dev/null
  fi
  [ "$(cast codesize "$addr" --rpc-url "$RPC")" != "0" ] || { echo "deploy failed at $addr" >&2; exit 1; }
  echo "$addr"
}

verifier=$(deploy "$(bytecode XMSSVerifier)")
args=$(cast abi-encode "constructor(address)" "$verifier")
factory=$(deploy "$(bytecode QDayAccountFactory)${args#0x}")
echo "XMSSVerifier        $verifier"
echo "QDayAccountFactory  $factory"
