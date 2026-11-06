#!/usr/bin/env bash
# Sends real transactions on the devnet and checks each result: an ETH transfer, token mints,
# transfer, approve and transferFrom. A throwaway second key (never saved) acts as the recipient.
# Needs: cast, ../../.env, and the tokens from deploy-tokens.sh.
set -euo pipefail
cd "$(dirname "$0")"
set -a; source ../../.env; set +a

RPC=$DEVNET_RPC_URL
USDC=0xd1108cBf3C5E7b44D7A58d4b2cdA492a0DDF6a72
WBTC=0xb212346871a9a08494e165F5AeFe655f06A5B75F
FAIL=0

check() { # name expected actual
  if [ "$2" = "$3" ]; then echo "  ok   $1"; else echo "  FAIL $1: expected $2, got $3"; FAIL=1; fi
}
send() { # key to sig args... -> prints "status gasUsed txhash"
  local key=$1; shift
  cast send --json --private-key "$key" --rpc-url "$RPC" "$@" \
    | python3 -c "import json,sys;r=json.load(sys.stdin);print(int(r['status'],16),int(r['gasUsed'],16),r['transactionHash'])"
}
bal() { cast call "$1" 'balanceOf(address)(uint256)' "$2" --rpc-url "$RPC" | awk '{print $1}'; }

BOB_KEY=$(cast wallet new --json | python3 -c "import json,sys;d=json.load(sys.stdin)['data'];d=d[0] if isinstance(d,list) else d;print(d['private_key'])")
BOB=$(cast wallet address "$BOB_KEY")
echo "team $TEAM_ADDRESS, throwaway recipient $BOB"

echo "1. ETH transfer (0.01 ETH)"
read -r st gas tx < <(send "$TEAM_PRIVATE_KEY" "$BOB" --value 0.01ether)
check "status ($tx, gas $gas)" 1 "$st"
check "recipient balance" 10000000000000000 "$(cast balance "$BOB" --rpc-url "$RPC")"

echo "2. Mint 1,000,000 USDC and 10 WBTC to the team"
u0=$(bal $USDC "$TEAM_ADDRESS"); w0=$(bal $WBTC "$TEAM_ADDRESS")
read -r st gas tx < <(send "$TEAM_PRIVATE_KEY" $USDC 'mint(address,uint256)' "$TEAM_ADDRESS" 1000000000000)
check "USDC mint ($tx, gas $gas)" 1 "$st"
read -r st gas tx < <(send "$TEAM_PRIVATE_KEY" $WBTC 'mint(address,uint256)' "$TEAM_ADDRESS" 1000000000)
check "WBTC mint ($tx, gas $gas)" 1 "$st"
check "team USDC +1,000,000" $((u0 + 1000000000000)) "$(bal $USDC "$TEAM_ADDRESS")"
check "team WBTC +10" $((w0 + 1000000000)) "$(bal $WBTC "$TEAM_ADDRESS")"

echo "3. Transfer 250 USDC and 0.5 WBTC to the recipient"
read -r st gas tx < <(send "$TEAM_PRIVATE_KEY" $USDC 'transfer(address,uint256)(bool)' "$BOB" 250000000)
check "USDC transfer ($tx, gas $gas)" 1 "$st"
read -r st gas tx < <(send "$TEAM_PRIVATE_KEY" $WBTC 'transfer(address,uint256)(bool)' "$BOB" 50000000)
check "WBTC transfer ($tx, gas $gas)" 1 "$st"
check "recipient USDC" 250000000 "$(bal $USDC "$BOB")"
check "recipient WBTC" 50000000 "$(bal $WBTC "$BOB")"

echo "4. Approve 100 USDC to the recipient, recipient pulls it with transferFrom"
read -r st gas tx < <(send "$TEAM_PRIVATE_KEY" $USDC 'approve(address,uint256)(bool)' "$BOB" 100000000)
check "approve ($tx, gas $gas)" 1 "$st"
read -r st gas tx < <(send "$BOB_KEY" $USDC 'transferFrom(address,address,uint256)(bool)' "$TEAM_ADDRESS" "$BOB" 100000000)
check "transferFrom signed by recipient ($tx, gas $gas)" 1 "$st"
check "recipient USDC after pull" 350000000 "$(bal $USDC "$BOB")"
check "allowance used up" 0 "$(cast call $USDC 'allowance(address,address)(uint256)' "$TEAM_ADDRESS" "$BOB" --rpc-url "$RPC" | awk '{print $1}')"

echo "5. Overspend is rejected"
if cast send --private-key "$BOB_KEY" --rpc-url "$RPC" $USDC 'transfer(address,uint256)(bool)' "$TEAM_ADDRESS" 999999999999 >/dev/null 2>&1; then
  check "overspend reverts" revert mined; else check "overspend reverts" revert revert; fi

echo
echo "team ETH $(cast balance --ether "$TEAM_ADDRESS" --rpc-url "$RPC")"
[ $FAIL = 0 ] && echo "ALL CHECKS PASSED" || { echo "SOME CHECKS FAILED"; exit 1; }
