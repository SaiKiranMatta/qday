#!/usr/bin/env bash
# Deploys the test USDC and WBTC tokens through the CREATE2 factory, so they land at the same
# addresses after every devnet reset. Safe to rerun: tokens already deployed are skipped.
# Needs: solc 0.8.28+, cast, and ../../.env (ask Sainath for the team key).
set -euo pipefail
cd "$(dirname "$0")"
set -a; source ../../.env; set +a

FACTORY=0x4e59b44847b379578588920cA78FbF26c0B4956C
SALT=$(cast keccak "qday-devnet-tokens-v1")
BIN=$(solc --optimize --evm-version cancun --bin TestERC20.sol 2>/dev/null | awk '/TestERC20 =======/{f=1} f&&/^Binary:/{getline; print; exit}')

deploy() {
  local name=$1 symbol=$2 decimals=$3
  local args init addr
  args=$(cast abi-encode "constructor(string,string,uint8)" "$name" "$symbol" "$decimals")
  init="0x${BIN}${args#0x}"
  addr=$(cast to-check-sum-address "0x$(cast keccak "0xff${FACTORY#0x}${SALT#0x}$(cast keccak "$init" | cut -c3-)" | cut -c27-)")
  if [ "$(cast codesize "$addr" --rpc-url "$DEVNET_RPC_URL")" = "0" ]; then
    cast send --quiet --private-key "$TEAM_PRIVATE_KEY" --rpc-url "$DEVNET_RPC_URL" \
      $FACTORY "${SALT}${init#0x}" >/dev/null
  fi
  echo "$symbol $addr $(cast call "$addr" 'symbol()(string)' --rpc-url "$DEVNET_RPC_URL") decimals=$(cast call "$addr" 'decimals()(uint8)' --rpc-url "$DEVNET_RPC_URL")"
}

deploy "USD Coin (QDay Test)" USDC 6
deploy "Wrapped BTC (QDay Test)" WBTC 8
