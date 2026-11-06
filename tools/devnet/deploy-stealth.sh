#!/usr/bin/env bash
# Deploys what ML-DSA accounts and stealth payments share, through the CREATE2 factory, so they land
# at the same addresses after every devnet reset. web/src/lib/chain.ts holds the addresses this prints.
# Safe to rerun: contracts already there are skipped. Costs about 100M gas in all, so it's for a devnet.
# Needs: forge, cast, cargo, and ../../.env (ask Sainath for the team key), or QDAY_RPC_URL and
# QDAY_DEPLOY_KEY. Run deploy-qday.sh first.
set -euo pipefail
cd "$(dirname "$0")"
if [ -z "${QDAY_RPC_URL:-}" ] || [ -z "${QDAY_DEPLOY_KEY:-}" ]; then set -a; source ../../.env; set +a; fi

ROOT=../..
CONTRACTS=$ROOT/contracts
MLDSA=$CONTRACTS/mldsa
CREATE2=0x4e59b44847b379578588920cA78FbF26c0B4956C
SALT=$(cast keccak "qday-devnet-v2")
RPC=${QDAY_RPC_URL:-$DEVNET_RPC_URL}
KEY=${QDAY_DEPLOY_KEY:-$TEAM_PRIVATE_KEY}
# A devnet block holds 60M gas, and code costs about 1,555 gas per byte. The gateway can't estimate fees.
DEPLOY_GAS=50000000
GAS_PRICE=2000000000
export ETH_RPC_TIMEOUT=30   # seconds per RPC call

(cd "$CONTRACTS" && forge build --quiet)
(cd "$MLDSA" && forge build --quiet)
bytecode() { python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['bytecode']['object'])" "$1/out/$2.sol/$2.json"; }

# Deploys init code through CREATE2 unless something already lives at its address.
deploy() {
  local init=$1 addr
  addr=$(cast to-check-sum-address "0x$(cast keccak "0xff${CREATE2#0x}${SALT#0x}$(cast keccak "$init" | cut -c3-)" | cut -c27-)")
  if [ "$(cast codesize "$addr" --rpc-url "$RPC")" = "0" ]; then
    cast send --quiet --private-key "$KEY" --rpc-url "$RPC" --legacy --gas-price "$GAS_PRICE" \
      --gas-limit "$DEPLOY_GAS" "$CREATE2" "${SALT}${init#0x}" >/dev/null
  fi
  [ "$(cast codesize "$addr" --rpc-url "$RPC")" != "0" ] || { echo "deploy failed at $addr" >&2; exit 1; }
  echo "$addr"
}

# Init code that makes the given bytes a contract's code, unchanged.
raw_init() {
  local runtime=${1#0x} size
  size=$(printf '%04x' $((${#runtime} / 2)))
  echo "0x61${size}600e60003961${size}6000f3${runtime}"
}

constructor() { cast abi-encode "constructor(address)" "$1" | cut -c3-; }

helper=$(deploy "$(raw_init "$(tr -d '\n' <"$MLDSA/helpers/f1600_170.hex")")")
matrix=$(deploy "$(raw_init "$(cargo run --quiet --release --manifest-path "$ROOT/Cargo.toml" -p qday-cli -- stealth-matrix)")")
verifier=$(deploy "$(bytecode "$MLDSA" MLDSA44Verifier)$(constructor "$helper")")
inline=$(deploy "$(bytecode "$MLDSA" MLDSA44InlineVerifier)$(constructor "$helper")")
factory=$(deploy "$(bytecode "$CONTRACTS" QDayMlDsaFactory)")
pool_args=$(cast abi-encode "constructor(address,address)" "$inline" "$matrix")
pool=$(deploy "$(bytecode "$CONTRACTS" QDayLatticePool)${pool_args#0x}")

echo "Keccak helper         $helper"
echo "Shared matrix         $matrix"
echo "MLDSA44Verifier       $verifier"
echo "MLDSA44InlineVerifier $inline"
echo "QDayMlDsaFactory      $factory"
echo "QDayLatticePool       $pool"
