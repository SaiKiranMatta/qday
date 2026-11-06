// Does the alt-mempool gateway accept a deploy frame first, and how much state gas may it use?
//   cd ~/Desktop/eth-mumbai/probes && npx tsx ~/Desktop/qday/tools/devnet/probes/deploycap.ts
// Sends [DEFAULT factory.createAccount(random key), VERIFY self, SENDER 1 gwei] from a fresh faucet-funded EOA
// with growing state limits on the deploy frame.
import { concat, encodeFunctionData, toHex, type Address, type Hex } from "viem";
import { generatePrivateKey, privateKeyToAccount } from "viem/accounts";
import * as lib from "@forshtat/hegota-support/tx";
import { toWire } from "@forshtat/hegota-support/rpc";
import { profile } from "@forshtat/hegota-support/profiles/ethrex-eip8141-tracer";

const GATEWAY = "https://altmempoolgateway.forshtat.com/rpc";
const NODE = "https://altmempool.forshtat.com";
const FACTORY = "0x31b18b0C1192c7AA62D69404d55b80B5182F7C6F" as Address;
const RECIPIENT = "0x8943545177806ED17B9F23F0a21ee5948eCaa776" as Address;

let id = 1;
async function rpc<T = any>(url: string, method: string, params: unknown[]): Promise<T> {
  const res = await fetch(url, {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: id++, method, params }, (_k, v) => (typeof v === "bigint" ? toHex(v) : v)),
  });
  const b = (await res.json()) as { result?: T; error?: { code: number; message: string } };
  if (b.error) throw new Error(`${b.error.code}: ${b.error.message}`);
  return b.result as T;
}
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const rand = (): Hex => toHex(crypto.getRandomValues(new Uint8Array(32)));

const createData = (root: Hex, seed: Hex) =>
  encodeFunctionData({
    abi: [{ type: "function", name: "createAccount", inputs: [{ type: "bytes32" }, { type: "bytes32" }], outputs: [{ type: "address" }], stateMutability: "nonpayable" }],
    functionName: "createAccount",
    args: [root, seed],
  });

const key = generatePrivateKey();
const eoa = privateKeyToAccount(key).address;
await rpc(GATEWAY, "skandha_explorer_faucet", [eoa]);
for (let i = 0; i < 30 && BigInt(await rpc<Hex>(NODE, "eth_getBalance", [eoa, "latest"])) === 0n; i++) await sleep(2000);
console.log("eoa", eoa);

for (const [exec, state] of [[2_000_000n, 500_000n], [2_000_000n, 10_000_000n], [10_000_000n, 10_000_000n]] as const) {
  const nonce = BigInt(await rpc<Hex>(NODE, "eth_getTransactionCount", [eoa, "latest"]));
  const tx = lib.newTx({
    chainId: 1337, sender: eoa, nonceSeq: nonce,
    frames: [
      lib.frame(profile, "DEFAULT", { target: FACTORY, execution: exec, state, data: createData(rand(), rand()) }),
      lib.frame(profile, "VERIFY", { scope: "EXECUTION_AND_PAYMENT", target: null, execution: 30_000n }),
      lib.frame(profile, "SENDER", { target: RECIPIENT, execution: 50_000n, state: 200_000n, value: 1_000_000_000n }),
    ],
    signatures: [lib.secp256k1Placeholder(profile)],
    fees: { maxFeePerGas: 2_000_000_000n },
  });
  try {
    const hash = await rpc<Hex>(GATEWAY, "eth_sendFrameTransaction", [toWire(await lib.signTx(profile, tx, key))]);
    let line = "not mined in 90 s";
    for (let i = 0; i < 45; i++) {
      const r: any = await rpc(NODE, "eth_getTransactionReceipt", [hash]).catch(() => null);
      if (r) {
        line = `mined block ${Number(r.blockNumber)} status ${r.status} gas ${Number(r.gasUsed)} frames ${JSON.stringify((r.frameReceipts ?? []).map((f: any) => [f.status, Number(f.gasUsed), Number(f.stateGasUsed ?? 0)]))}`;
        break;
      }
      await sleep(2000);
    }
    console.log(`deploy exec ${exec} state ${state}: ADMITTED ${hash} ${line}`);
  } catch (e) {
    console.log(`deploy exec ${exec} state ${state}: REJECTED ${(e as Error).message}`);
  }
}
void concat;
