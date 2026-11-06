import { loadFrame } from "@qday/frame";
import { type Hex, keccak256 } from "viem";
import { GAS_PRICE, MINE_TIMEOUT_MS, ML_FACTORY, ML_VERIFIER, STAKING_REGISTRY, client, devWallet } from "./chain";
import type { Address } from "./tokens";

// An ML-DSA account reads its key from a data contract, so the factory deploys two things. The account's
// address follows from them, and from the stake it was created with.

/** The sidecar's minimum is 0.01 ETH. */
const STAKE = 12_000_000_000_000_000n;
const UNSTAKE_DELAY = 86_400;
const SALT = `0x${"00".repeat(32)}` as const;
/** Measured on the devnet: about 32M for the key and 5.4M for the account. */
const BLOB_GAS = 40_000_000n;
const ACCOUNT_GAS = 8_000_000n;

const settings = [
  { type: "address" },
  { type: "bytes32" },
  { type: "address" },
  { type: "bytes32" },
  { type: "address" },
  { type: "uint256" },
  { type: "uint32" },
  { type: "bytes32" },
] as const;

const factoryAbi = [
  { type: "function", name: "predictBlob", stateMutability: "view", inputs: [{ type: "bytes" }], outputs: [{ type: "address" }] },
  { type: "function", name: "deployBlob", stateMutability: "nonpayable", inputs: [{ type: "bytes" }], outputs: [{ type: "address" }] },
  { type: "function", name: "predictAccount", stateMutability: "view", inputs: settings, outputs: [{ type: "address" }] },
  { type: "function", name: "deployAccount", stateMutability: "payable", inputs: settings, outputs: [{ type: "address" }] },
] as const;

interface MlAccountPlan {
  blob: Hex;
  blobAt: Address;
  address: Address;
  args: readonly [Address, Hex, Address, Hex, Address, bigint, number, Hex];
}

const plans = new Map<Hex, Promise<MlAccountPlan>>();

async function plan(publicKey: Hex): Promise<MlAccountPlan> {
  if (!STAKING_REGISTRY) throw new Error("ML-DSA accounts need VITE_QDAY_STAKING_REGISTRY set for this devnet.");
  const code = await client.getCode({ address: ML_VERIFIER });
  if (!code) throw new Error("The ML-DSA contracts aren't on this network. Run tools/devnet/deploy-stealth.sh.");
  const blob = (await loadFrame()).mlBlob(publicKey);
  const blobAt = await client.readContract({ address: ML_FACTORY, abi: factoryAbi, functionName: "predictBlob", args: [blob] });
  const args = [ML_VERIFIER, keccak256(code), blobAt, keccak256(blob), STAKING_REGISTRY, STAKE, UNSTAKE_DELAY, SALT] as const;
  const address = await client.readContract({ address: ML_FACTORY, abi: factoryAbi, functionName: "predictAccount", args });
  return { blob, blobAt, address, args };
}

/** Where an ML-DSA key's account will be. Remembered per key, because working it out sends the 20 KB key. */
export function planMlAccount(publicKey: Hex): Promise<MlAccountPlan> {
  const known = plans.get(publicKey);
  if (known) return known;
  const planning = plan(publicKey);
  plans.set(publicKey, planning);
  planning.catch(() => plans.delete(publicKey));
  return planning;
}

/** Deploys the key and the account with the dev key, and returns the block. Safe to retry. */
export async function deployMlAccount(publicKey: Hex): Promise<number> {
  const { blob, blobAt, args } = await planMlAccount(publicKey);
  const wallet = devWallet();
  const fees = { type: "legacy", gasPrice: GAS_PRICE } as const;
  const mined = async (hash: Hex, what: string) => {
    const receipt = await client.waitForTransactionReceipt({ hash, timeout: MINE_TIMEOUT_MS });
    if (receipt.status !== "success") throw new Error(`${what} failed in block ${receipt.blockNumber}.`);
    return Number(receipt.blockNumber);
  };
  if (!(await client.getCode({ address: blobAt }))) {
    const hash = await wallet.writeContract({
      address: ML_FACTORY,
      abi: factoryAbi,
      functionName: "deployBlob",
      args: [blob],
      gas: BLOB_GAS,
      ...fees,
    });
    await mined(hash, "Deploying the key");
  }
  const hash = await wallet.writeContract({
    address: ML_FACTORY,
    abi: factoryAbi,
    functionName: "deployAccount",
    args,
    value: STAKE,
    gas: ACCOUNT_GAS,
    ...fees,
  });
  return mined(hash, "Deploying the account");
}
