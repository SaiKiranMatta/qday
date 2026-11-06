import { createPublicClient, createWalletClient, defineChain, erc20Abi, formatUnits, http, type Hex } from "viem";
import { privateKeyToAccount } from "viem/accounts";
import type { Address, Token } from "./tokens";

/** Every RPC call gives up after this long, so nothing in the app hangs on a dead endpoint. */
export const REQUEST_TIMEOUT_MS = 15_000;
/** How long to wait for a submitted transaction to be mined. */
export const MINE_TIMEOUT_MS = 120_000;
/** How often to check for the receipt of a submitted transaction. */
const RECEIPT_POLL_MS = 500;

interface Network {
  name: string;
  rpc: string;
  gateway: string;
  /** Where test ETH comes from: the gateway's faucet, or a funded dev key on a private chain. */
  faucet: { kind: "gateway" } | { kind: "key"; key: Hex };
  /** Seconds between blocks, for telling people when the next one is due. */
  blockSeconds: number;
}

const NETWORKS: Record<string, Network> = {
  local: {
    name: "Local devnet",
    rpc: "http://127.0.0.1:8545",
    gateway: "http://127.0.0.1:14337/rpc",
    // The local devnet's premine key. Throwaway by design: it's published with the devnet setup.
    faucet: { kind: "key", key: "0x06f911c1ecb2053bb443ba0a0e28d630c583c3c0c2f334a9ecdc8c7c6ad4ea15" },
    blockSeconds: 6,
  },
  public: {
    name: "Public devnet",
    rpc: "https://altmempool.forshtat.com",
    gateway: "https://altmempoolgateway.forshtat.com/rpc",
    faucet: { kind: "gateway" },
    blockSeconds: 12,
  },
};

/** Pick with VITE_QDAY_NETWORK=local|public. Local is the default: the public gateway stalls often. */
export const NETWORK: Network = NETWORKS[import.meta.env.VITE_QDAY_NETWORK ?? "local"] ?? NETWORKS.local!;
export const RPC_URL = NETWORK.rpc;
export const GATEWAY_URL = NETWORK.gateway;

/** Same address on every devnet: tools/devnet/deploy-qday.sh deploys it through CREATE2. */
export const FACTORY: Address = "0xbCebAd3eCE45625983409B429406a5Dc8d38f876";

export const devnet = defineChain({
  id: 1337,
  name: NETWORK.name,
  nativeCurrency: { name: "Ether", symbol: "ETH", decimals: 18 },
  rpcUrls: { default: { http: [RPC_URL] } },
});

export const client = createPublicClient({
  chain: devnet,
  transport: http(RPC_URL, { timeout: REQUEST_TIMEOUT_MS }),
  pollingInterval: RECEIPT_POLL_MS,
});

export const accountAbi = [
  { type: "function", name: "root", stateMutability: "view", inputs: [], outputs: [{ type: "bytes32" }] },
  { type: "function", name: "nextIndex", stateMutability: "view", inputs: [], outputs: [{ type: "uint256" }] },
  { type: "function", name: "tree", stateMutability: "view", inputs: [], outputs: [{ type: "uint256" }] },
] as const;

const factoryAbi = [
  {
    type: "function",
    name: "accountOf",
    stateMutability: "view",
    inputs: [{ type: "bytes32" }, { type: "bytes32" }],
    outputs: [{ type: "address" }],
  },
] as const;

export interface OnChainAccount {
  address: Address;
  /** False until the account's first transaction deploys it. */
  deployed: boolean;
  nonce: number;
  nextIndex: number | null;
  balanceWei: bigint;
  /** Formatted balances by token id. */
  balances: Record<string, string>;
  /** Exact balances by token id, in each token's smallest unit. */
  units: Record<string, bigint>;
}

export function formatAmount(value: bigint, decimals: number): string {
  const [whole = "0", fraction = ""] = formatUnits(value, decimals).split(".");
  const trimmed = fraction.slice(0, 4).replace(/0+$/, "");
  return `${BigInt(whole).toLocaleString("en-US")}${trimmed ? `.${trimmed}` : ""}`;
}

/** The account for a device key: deployed or not, its address follows from the key. */
export async function accountAddress(root: Hex, pubSeed: Hex): Promise<Address> {
  return client.readContract({ address: FACTORY, abi: factoryAbi, functionName: "accountOf", args: [root, pubSeed] });
}

/** Balances and key state. Token reads that fail count as 0. */
export async function readAccount(address: Address, tokens: Token[]): Promise<OnChainAccount> {
  const [code, nonce, ...balances] = await Promise.all([
    client.getCode({ address }),
    client.getTransactionCount({ address }),
    ...tokens.map((token) =>
      token.address === null
        ? client.getBalance({ address })
        : client
            .readContract({ address: token.address, abi: erc20Abi, functionName: "balanceOf", args: [address] })
            .catch(() => 0n),
    ),
  ]);
  const deployed = !!code && code !== "0x";
  const nextIndex = deployed
    ? await client.readContract({ address, abi: accountAbi, functionName: "nextIndex" }).catch(() => null)
    : null;
  const native = tokens.findIndex((t) => t.address === null);
  return {
    address,
    deployed,
    nonce,
    nextIndex: nextIndex === null ? null : Number(nextIndex),
    balanceWei: native >= 0 ? (balances[native] ?? 0n) : 0n,
    balances: Object.fromEntries(tokens.map((token, i) => [token.id, formatAmount(balances[i] ?? 0n, token.decimals)])),
    units: Object.fromEntries(tokens.map((token, i) => [token.id, balances[i] ?? 0n])),
  };
}

async function rpc<T>(url: string, method: string, params: unknown[]): Promise<T> {
  const res = await fetch(url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }),
    signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
  });
  const body = (await res.json()) as { result?: T; error?: { message: string } };
  if (body.error) throw new Error(body.error.message);
  return body.result as T;
}

/** Sends test ETH to an address, from the network's faucet. */
export async function requestTestEth(to: Address): Promise<void> {
  if (NETWORK.faucet.kind === "gateway") {
    await rpc(GATEWAY_URL, "skandha_explorer_faucet", [to]);
    return;
  }
  const wallet = createWalletClient({
    account: privateKeyToAccount(NETWORK.faucet.key),
    chain: devnet,
    transport: http(RPC_URL, { timeout: REQUEST_TIMEOUT_MS }),
  });
  // The gateway can't estimate fees, so plain transactions use a fixed legacy price.
  const hash = await wallet.sendTransaction({ to, value: 10n ** 18n, type: "legacy", gasPrice: 2_000_000_000n });
  await client.waitForTransactionReceipt({ hash, timeout: MINE_TIMEOUT_MS });
}

/** True where the app can mint test tokens itself: a private devnet with a funded dev key. */
export const CAN_MINT_TEST_TOKENS = NETWORK.faucet.kind === "key";

const mintAbi = [
  { type: "function", name: "mint", stateMutability: "nonpayable", inputs: [{ type: "address" }, { type: "uint256" }], outputs: [] },
] as const;

/** Mints a round amount of each test token (anyone can mint the devnet's TestERC20) to an address. */
export async function requestTestTokens(to: Address, tokens: Token[]): Promise<void> {
  if (NETWORK.faucet.kind !== "key") throw new Error("Test tokens are only minted on the local devnet.");
  const wallet = createWalletClient({
    account: privateKeyToAccount(NETWORK.faucet.key),
    chain: devnet,
    transport: http(RPC_URL, { timeout: REQUEST_TIMEOUT_MS }),
  });
  for (const token of tokens) {
    if (!token.address || token.custom) continue;
    // 1,000 of a dollar-style token, 1 of anything finer (1 WBTC, 1 of an 18-decimal token).
    const amount = 10n ** BigInt(token.decimals) * (token.decimals <= 6 ? 1000n : 1n);
    const hash = await wallet.writeContract({
      address: token.address,
      abi: mintAbi,
      functionName: "mint",
      args: [to, amount],
      type: "legacy",
      gasPrice: 2_000_000_000n,
    });
    await client.waitForTransactionReceipt({ hash, timeout: MINE_TIMEOUT_MS });
  }
}

export interface FrameReceipt {
  hash: Hex;
  block: number;
  ok: boolean;
  gasUsed: bigint;
}

/** Time between blocks on this network, in milliseconds. */
export const BLOCK_MS = NETWORK.blockSeconds * 1000;

/** Hands a signed frame transaction to the gateway. Resolves with its hash once the gateway accepts it. */
export async function broadcastFrameTx(submission: Record<string, unknown>): Promise<Hex> {
  const hash = await rpc<Hex>(GATEWAY_URL, "eth_sendFrameTransaction", [submission]);
  console.info("[qday] submitted", hash);
  return hash;
}

/** When the next block is due (epoch ms), from the latest block's timestamp. Later slots follow every BLOCK_MS. */
export async function nextBlockAt(): Promise<number> {
  const latest = await rpc<{ timestamp: Hex }>(RPC_URL, "eth_getBlockByNumber", ["latest", false]);
  return Number(BigInt(latest.timestamp)) * 1000 + BLOCK_MS;
}

/** Waits, with a limit, for a submitted transaction to be mined. */
export async function waitForFrameReceipt(hash: Hex): Promise<FrameReceipt> {
  const deadline = Date.now() + MINE_TIMEOUT_MS;
  while (Date.now() < deadline) {
    const receipt = await rpc<{ blockNumber: Hex; status: Hex; gasUsed: Hex } | null>(RPC_URL, "eth_getTransactionReceipt", [hash]).catch(
      () => null,
    );
    if (receipt) {
      const result = { hash, block: Number(receipt.blockNumber), ok: receipt.status === "0x1", gasUsed: BigInt(receipt.gasUsed) };
      console.info("[qday] mined", result);
      return result;
    }
    await new Promise((r) => setTimeout(r, RECEIPT_POLL_MS));
  }
  throw new Error(`Not mined within ${MINE_TIMEOUT_MS / 1000}s. The gateway may be stalled. Transaction ${hash}`);
}

/** Submits a signed frame transaction and waits for it to be mined. */
export async function submitFrameTx(submission: Record<string, unknown>): Promise<FrameReceipt> {
  return waitForFrameReceipt(await broadcastFrameTx(submission));
}
