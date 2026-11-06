import { DemoSigner, type DemoRequest, type DemoStorage, loadFrame } from "@qday/frame";
import type { Hex } from "viem";
import { accountAbi, accountAddress, client } from "./chain";

const LOCK_NAME = "qday.demo.sign";
const SEED_KEY = "qday.demo.seed";

// A read or write that fails acts as if nothing is stored. The signer checks each write by reading it back.
const storage: DemoStorage = {
  get: (key) => {
    try {
      return localStorage.getItem(key);
    } catch {
      return null;
    }
  },
  set: (key, value) => {
    try {
      localStorage.setItem(key, value);
    } catch {
      // The signer finds this out when it reads the value back.
    }
  },
  remove: (key) => {
    try {
      localStorage.removeItem(key);
    } catch {
      // Nothing can be stored, so nothing is left behind.
    }
  },
};

/** Whether this browser already holds a demo wallet. */
export const hasDemoWallet = () => storage.get(SEED_KEY) !== null;

let queue: Promise<unknown> = Promise.resolve();

/** One signature at a time across tabs, or within this tab where the browser has no Web Locks. */
function lock<T>(work: () => Promise<T>): Promise<T> {
  if (navigator.locks) return navigator.locks.request(LOCK_NAME, work) as Promise<T>;
  const run = queue.then(work, work);
  queue = run.catch(() => {});
  return run;
}

async function chainNextIndex({ root, pubSeed }: { root: Hex; pubSeed: Hex }): Promise<number> {
  const address = await accountAddress(root, pubSeed);
  if (!(await client.getCode({ address }))) return 0;
  return Number(await client.readContract({ address, abi: accountAbi, functionName: "nextIndex" }));
}

/** Opens the demo wallet in this browser, or the one `seed` belongs to. */
export async function openDemo(approve: (request: DemoRequest) => Promise<boolean>, seed?: Hex): Promise<DemoSigner> {
  return DemoSigner.open(
    {
      codec: await loadFrame(),
      storage,
      chainId: () => client.getChainId(),
      chainNextIndex,
      lock,
      approve,
      random: (bytes) => crypto.getRandomValues(new Uint8Array(bytes)),
    },
    seed,
  );
}
