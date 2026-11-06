#!/bin/sh
# Deploys the XMSS verifier, the QDay account factory and the USDC/WBTC test tokens through the
# CREATE2 factory once the chain is producing blocks. The init code is exactly what gives them their
# usual addresses, so the web app needs no new configuration. Safe to rerun: deployed ones are skipped.
# Then stays up, healthy once everything is deployed.
set -u
RPC=${RPC_URL:-http://ethrex:8545}
KEY=0x06f911c1ecb2053bb443ba0a0e28d630c583c3c0c2f334a9ecdc8c7c6ad4ea15   # the devnet's public premine key
CREATE2=0x4e59b44847b379578588920cA78FbF26c0B4956C
. /initcode/salts.env
export ETH_RPC_TIMEOUT=30

deploy() { # name salt
  init=$(cat "/initcode/$1.hex")
  addr=$(cast to-check-sum-address "0x$(cast keccak "0xff${CREATE2#0x}${2#0x}$(cast keccak "$init" | cut -c3-)" | cut -c27-)")
  if [ "$(cast codesize "$addr" --rpc-url "$RPC")" = "0" ]; then
    cast send --quiet --private-key "$KEY" --rpc-url "$RPC" "$CREATE2" "${2}${init#0x}" >/dev/null || return 1
  fi
  [ "$(cast codesize "$addr" --rpc-url "$RPC")" != "0" ] || return 1
  echo "$1 $addr"
}

rm -f /tmp/deployed
until [ "$(cast block-number --rpc-url "$RPC" 2>/dev/null || echo 0)" -gt 2 ] \
  && [ "$(cast codesize "$CREATE2" --rpc-url "$RPC" 2>/dev/null || echo 0)" != "0" ]; do
  echo "waiting for the chain to produce blocks"
  sleep 10
done
until deploy XMSSVerifier "$QDAY_SALT" && deploy QDayAccountFactory "$QDAY_SALT" \
  && deploy USDC "$TOKEN_SALT" && deploy WBTC "$TOKEN_SALT"; do
  echo "deploy didn't finish; retrying in 15 s"
  sleep 15
done
touch /tmp/deployed
echo "all contracts deployed"
exec sleep 2147483647
