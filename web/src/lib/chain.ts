import { createPublicClient, defineChain, erc20Abi, formatUnits, http } from "viem";
import type { Address, Token } from "./tokens";

/** Devnet endpoints. Override with VITE_RPC_URL (reads) and VITE_GATEWAY_URL (frame transactions). */
export const RPC_URL: string = import.meta.env.VITE_RPC_URL ?? "https://altmempool.forshtat.com";
export const GATEWAY_URL: string = import.meta.env.VITE_GATEWAY_URL ?? "https://altmempoolgateway.forshtat.com/rpc";

export const devnet = defineChain({
  id: 1337,
  name: "QDay devnet",
  nativeCurrency: { name: "Ether", symbol: "ETH", decimals: 18 },
  rpcUrls: { default: { http: [RPC_URL] } },
});

export const client = createPublicClient({ chain: devnet, transport: http(RPC_URL) });

/** What the QDay account contract exposes. */
export const accountAbi = [
  { type: "function", name: "root", stateMutability: "view", inputs: [], outputs: [{ type: "bytes32" }] },
  { type: "function", name: "pubSeed", stateMutability: "view", inputs: [], outputs: [{ type: "bytes32" }] },
  { type: "function", name: "nextIndex", stateMutability: "view", inputs: [], outputs: [{ type: "uint256" }] },
  { type: "function", name: "tree", stateMutability: "view", inputs: [], outputs: [{ type: "uint256" }] },
] as const;

export interface OnChainAccount {
  isContract: boolean;
  root: `0x${string}` | null;
  nextIndex: number | null;
  tree: number | null;
  /** Formatted balances by token id. */
  balances: Record<string, string>;
}

function display(value: bigint, decimals: number): string {
  const text = formatUnits(value, decimals);
  const [whole = "0", fraction = ""] = text.split(".");
  const trimmed = fraction.slice(0, 4).replace(/0+$/, "");
  return `${BigInt(whole).toLocaleString("en-US")}${trimmed ? `.${trimmed}` : ""}`;
}

/** Reads balances and, if it's a QDay account, its key state. Failing reads come back as null. */
export async function readAccount(address: Address, tokens: Token[]): Promise<OnChainAccount> {
  const [code, ...balances] = await Promise.all([
    client.getCode({ address }),
    ...tokens.map((token) =>
      token.address === null
        ? client.getBalance({ address })
        : client
            .readContract({ address: token.address, abi: erc20Abi, functionName: "balanceOf", args: [address] })
            .catch(() => 0n),
    ),
  ]);
  const isContract = !!code && code !== "0x";
  const read = <T,>(fn: "root" | "nextIndex" | "tree") =>
    isContract
      ? (client.readContract({ address, abi: accountAbi, functionName: fn }) as Promise<T>).catch(() => null)
      : Promise.resolve(null);
  const [root, nextIndex, tree] = await Promise.all([read<`0x${string}`>("root"), read<bigint>("nextIndex"), read<bigint>("tree")]);

  return {
    isContract,
    root,
    nextIndex: nextIndex === null ? null : Number(nextIndex),
    tree: tree === null ? null : Number(tree),
    balances: Object.fromEntries(tokens.map((token, i) => [token.id, display(balances[i] ?? 0n, token.decimals)])),
  };
}
