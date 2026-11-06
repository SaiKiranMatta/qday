#!/usr/bin/env bash
# Regenerates initcode/ from the contracts, after a contract changes. Run from the repo with forge, cast,
# cargo and solc 0.8.28 installed, and the sidecar repo cloned where tools/devnet/local/up.sh clones it
# (for the staking registry). The addresses it prints are where the hosted devnet will put them.
set -euo pipefail
cd "$(dirname "$0")/../../.."
SIDECAR_REPO=${SIDECAR_REPO:-$HOME/Desktop/eth-mumbai/mempool-workshop-sidecar}
(cd contracts && forge build --quiet)
(cd contracts/mldsa && forge build --quiet)
OUT=deploy/coolify/deployer/initcode
CREATE2=0x4e59b44847b379578588920cA78FbF26c0B4956C
QDAY_SALT=$(cast keccak "qday-devnet-v2")
TOKEN_SALT=$(cast keccak "qday-devnet-tokens-v1")
artifact() { python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['bytecode']['object'])" "${2:-contracts}/out/$1.sol/$1.json"; }
address() { cast to-check-sum-address "0x$(cast keccak "0xff${CREATE2#0x}${1#0x}$(cast keccak "$2" | cut -c3-)" | cut -c27-)"; }
# Init code that makes the given bytes a contract's code, unchanged (as tools/devnet/deploy-stealth.sh does).
raw_init() { local runtime=${1#0x}; local size; size=$(printf '%04x' $((${#runtime} / 2))); echo "0x61${size}600e60003961${size}6000f3${runtime}"; }
args() { cast abi-encode "$@" | cut -c3-; }

verifier=$(artifact XMSSVerifier)
factory="$(artifact QDayAccountFactory)$(args "constructor(address)" "$(address "$QDAY_SALT" "$verifier")")"
bin=$(cd tools/devnet && solc --optimize --evm-version cancun --bin TestERC20.sol 2>/dev/null | awk '/TestERC20 =======/{f=1} f&&/^Binary:/{getline; print; exit}')
usdc="0x${bin}$(args "constructor(string,string,uint8)" "USD Coin (QDay Test)" USDC 6)"
wbtc="0x${bin}$(args "constructor(string,string,uint8)" "Wrapped BTC (QDay Test)" WBTC 8)"

# What ML-DSA accounts and private payments share, as tools/devnet/deploy-stealth.sh deploys it.
helper=$(raw_init "$(tr -d '\n' < contracts/mldsa/helpers/f1600_170.hex)")
matrix=$(raw_init "$(cargo run --quiet --release -p qday-cli -- stealth-matrix)")
ml_verifier="$(artifact MLDSA44Verifier contracts/mldsa)$(args "constructor(address)" "$(address "$QDAY_SALT" "$helper")")"
inline="$(artifact MLDSA44InlineVerifier contracts/mldsa)$(args "constructor(address)" "$(address "$QDAY_SALT" "$helper")")"
ml_factory=$(artifact QDayMlDsaFactory)
pool="$(artifact QDayLatticePool)$(args "constructor(address,address)" "$(address "$QDAY_SALT" "$inline")" "$(address "$QDAY_SALT" "$matrix")")"
# The sidecar's NaiveStakingRegistry (MIT), where ML-DSA accounts stake.
registry=$(grep -o 'STAKING_REGISTRY_INIT_CODE: Hex = "0x[0-9a-f]*"' \
  "$SIDECAR_REPO/deploy/predeploy/src/protocols/staking-registry-deployment.ts" | grep -o '0x[0-9a-f]*')

write() { printf '%s\n' "$2" > "$OUT/$1.hex"; printf '%-22s %s\n' "$1" "$(address "$3" "$2")"; }
write XMSSVerifier "$verifier" "$QDAY_SALT"
write QDayAccountFactory "$factory" "$QDAY_SALT"
write USDC "$usdc" "$TOKEN_SALT"
write WBTC "$wbtc" "$TOKEN_SALT"
write KeccakHelper "$helper" "$QDAY_SALT"
write StealthMatrix "$matrix" "$QDAY_SALT"
write MLDSA44Verifier "$ml_verifier" "$QDAY_SALT"
write MLDSA44InlineVerifier "$inline" "$QDAY_SALT"
write QDayMlDsaFactory "$ml_factory" "$QDAY_SALT"
write QDayLatticePool "$pool" "$QDAY_SALT"
write StakingRegistry "$registry" "$QDAY_SALT"
printf 'QDAY_SALT=%s\nTOKEN_SALT=%s\n' "$QDAY_SALT" "$TOKEN_SALT" > "$OUT/salts.env"
