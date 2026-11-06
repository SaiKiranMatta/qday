#!/usr/bin/env bash
# Regenerates initcode/ from the contracts, after a contract changes. Run from the repo with forge, cast
# and solc 0.8.28 installed. The addresses it prints are where the hosted devnet will put them.
set -euo pipefail
cd "$(dirname "$0")/../../.."
(cd contracts && forge build --quiet)
OUT=deploy/coolify/deployer/initcode
CREATE2=0x4e59b44847b379578588920cA78FbF26c0B4956C
QDAY_SALT=$(cast keccak "qday-devnet-v2")
TOKEN_SALT=$(cast keccak "qday-devnet-tokens-v1")
artifact() { python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['bytecode']['object'])" "contracts/out/$1.sol/$1.json"; }
address() { cast to-check-sum-address "0x$(cast keccak "0xff${CREATE2#0x}${1#0x}$(cast keccak "$2" | cut -c3-)" | cut -c27-)"; }
verifier=$(artifact XMSSVerifier)
factory="$(artifact QDayAccountFactory)$(cast abi-encode "constructor(address)" "$(address "$QDAY_SALT" "$verifier")" | cut -c3-)"
bin=$(cd tools/devnet && solc --optimize --evm-version cancun --bin TestERC20.sol 2>/dev/null | awk '/TestERC20 =======/{f=1} f&&/^Binary:/{getline; print; exit}')
usdc="0x${bin}$(cast abi-encode "constructor(string,string,uint8)" "USD Coin (QDay Test)" USDC 6 | cut -c3-)"
wbtc="0x${bin}$(cast abi-encode "constructor(string,string,uint8)" "Wrapped BTC (QDay Test)" WBTC 8 | cut -c3-)"
printf '%s\n' "$verifier" > "$OUT/XMSSVerifier.hex"
printf '%s\n' "$factory" > "$OUT/QDayAccountFactory.hex"
printf '%s\n' "$usdc" > "$OUT/USDC.hex"
printf '%s\n' "$wbtc" > "$OUT/WBTC.hex"
printf 'QDAY_SALT=%s\nTOKEN_SALT=%s\n' "$QDAY_SALT" "$TOKEN_SALT" > "$OUT/salts.env"
echo "XMSSVerifier        $(address "$QDAY_SALT" "$verifier")"
echo "QDayAccountFactory  $(address "$QDAY_SALT" "$factory")"
echo "USDC                $(address "$TOKEN_SALT" "$usdc")"
echo "WBTC                $(address "$TOKEN_SALT" "$wbtc")"
