import type { Hex } from "viem";
import { FACTORY, REQUEST_TIMEOUT_MS, RPC_URL } from "./chain";
import { type Address, KNOWN_TOKENS, type Token } from "./tokens";

// The devnets have no block explorer, and the usual ones don't understand frame transactions, so the
// app reads transactions from the node itself. VITE_QDAY_EXPLORER points links at another explorer
// instead, as a template like "https://explorer.example/tx/{hash}".

const TEMPLATE: string = import.meta.env.VITE_QDAY_EXPLORER ?? "/tx/{hash}";

/** Where a transaction's explorer page lives. */
export function txUrl(hash: string): string {
  return TEMPLATE.replace("{hash}", hash);
}

/** The hash on a built-in explorer path, if this page is one. */
export function txHashFromPath(path: string): Hex | null {
  const match = /^\/tx\/(0x[0-9a-fA-F]{64})\/?$/.exec(path);
  return match ? (match[1] as Hex) : null;
}

const SELECTORS = {
  validate: "0x6901f668",
  spend: "0x92f59d73",
  rotate: "0xed296e9c",
  create: "0x183815c8",
  transfer: "0xa9059cbb",
  mint: "0x40c10f19",
} as const;
const TRANSFER_TOPIC = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

interface RpcFrame {
  mode: Hex;
  to: Address | null;
  value: Hex;
  data: Hex;
  gasLimit: Hex;
  stateLimit?: Hex;
}

interface RpcTx {
  hash: Hex;
  type: Hex;
  from: Address;
  to?: Address | null;
  sender?: Address;
  value?: Hex;
  input?: Hex;
  gas?: Hex;
  nonce?: Hex;
  nonceSeq?: Hex;
  maxFeePerGas?: Hex;
  gasPrice?: Hex;
  blockNumber: Hex | null;
  frames?: RpcFrame[];
  signatures?: { signature: Hex }[];
}

interface RpcLog {
  address: Address;
  topics: Hex[];
  data: Hex;
}

interface RpcReceipt {
  status: Hex;
  blockNumber: Hex;
  gasUsed: Hex;
  effectiveGasPrice: Hex;
  payer?: Address;
  logs: RpcLog[];
  frameReceipts?: { gasUsed: Hex; status: Hex }[];
}

async function call<T>(method: string, params: unknown[]): Promise<T> {
  const res = await fetch(RPC_URL, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }),
    signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
  });
  const body = (await res.json()) as { result?: T; error?: { message: string } };
  if (body.error) throw new Error(body.error.message);
  return body.result as T;
}

export type FrameMode = "default" | "verify" | "sender" | "unknown";

/** One frame, with what it does in words. */
export interface FrameDetail {
  mode: FrameMode;
  target: Address | null;
  value: bigint;
  data: Hex;
  gasLimit: bigint;
  stateLimit: bigint;
  gasUsed: bigint | null;
  ok: boolean | null;
  does: Action;
}

/** What a frame or a plain transaction does. Screens turn this into words. */
export type Action =
  | { kind: "verify" }
  | { kind: "spend"; key: number }
  | { kind: "rotate" }
  | { kind: "create" }
  | { kind: "send"; to: Address; value: bigint }
  | { kind: "token"; token: Address; to: Address; amount: bigint }
  | { kind: "mint"; token: Address; to: Address; amount: bigint }
  | { kind: "call"; target: Address | null; selector: Hex | null };

export interface TokenMove {
  token: Address;
  from: Address;
  to: Address;
  amount: bigint;
}

export interface TxDetail {
  hash: Hex;
  status: "confirmed" | "failed" | "pending";
  frameTx: boolean;
  /** The account a frame transaction runs as, or a plain transaction's sender. */
  from: Address;
  /** Who paid the fee, when that isn't `from`. */
  payer: Address | null;
  nonce: number | null;
  block: number | null;
  time: Date | null;
  gasUsed: bigint | null;
  gasLimit: bigint;
  feePaid: bigint | null;
  maxFeePerGas: bigint;
  signatureBytes: number | null;
  /** The transaction's point: the last action frame, or the plain transaction itself. */
  main: Action;
  frames: FrameDetail[];
  tokenMoves: TokenMove[];
}

const lower = (a: string | null | undefined) => (a ?? "").toLowerCase();
const word = (data: Hex, n: number) => `0x${data.slice(10 + n * 64, 10 + (n + 1) * 64)}`;
const wordAddress = (data: Hex, n: number) => `0x${data.slice(10 + n * 64 + 24, 10 + (n + 1) * 64)}` as Address;

/** Reads a call's meaning from its target, value and data. */
function decode(mode: FrameMode, target: Address | null, value: bigint, data: Hex): Action {
  const selector = data.length >= 10 ? (data.slice(0, 10).toLowerCase() as Hex) : null;
  if (mode === "verify") return { kind: "verify" };
  if (mode === "default" && lower(target) === lower(FACTORY) && selector === SELECTORS.create) return { kind: "create" };
  if (target === null && selector === SELECTORS.spend) return { kind: "spend", key: Number(BigInt(word(data, 0))) };
  if (target === null && selector === SELECTORS.rotate) return { kind: "rotate" };
  if (target && selector === SELECTORS.transfer && data.length === 138) {
    return { kind: "token", token: target, to: wordAddress(data, 0), amount: BigInt(word(data, 1)) };
  }
  if (target && selector === SELECTORS.mint && data.length === 138) {
    return { kind: "mint", token: target, to: wordAddress(data, 0), amount: BigInt(word(data, 1)) };
  }
  if (target && data === "0x" && value > 0n) return { kind: "send", to: target, value };
  return { kind: "call", target, selector };
}

const MODES: Record<number, FrameMode> = { 0: "default", 1: "verify", 2: "sender" };

/** A transaction as the explorer shows it, or null when the node doesn't know it (yet). */
export async function loadTx(hash: Hex): Promise<TxDetail | null> {
  const tx = await call<RpcTx | null>("eth_getTransactionByHash", [hash]);
  if (!tx) return null;
  const receipt = tx.blockNumber ? await call<RpcReceipt | null>("eth_getTransactionReceipt", [hash]) : null;
  const block = receipt ? Number(BigInt(receipt.blockNumber)) : null;
  const header = block !== null ? await call<{ timestamp: Hex } | null>("eth_getBlockByNumber", [receipt!.blockNumber, false]) : null;

  const frameTx = BigInt(tx.type) === 6n;
  const frames: FrameDetail[] = (tx.frames ?? []).map((f, i) => {
    const mode = MODES[Number(BigInt(f.mode))] ?? "unknown";
    const value = BigInt(f.value);
    const result = receipt?.frameReceipts?.[i];
    return {
      mode,
      target: f.to,
      value,
      data: f.data,
      gasLimit: BigInt(f.gasLimit),
      stateLimit: BigInt(f.stateLimit ?? "0x0"),
      gasUsed: result ? BigInt(result.gasUsed) : null,
      ok: result ? result.status === "0x1" : null,
      does: decode(mode, f.to, value, f.data),
    };
  });

  const from = tx.sender ?? tx.from;
  let main: Action;
  if (frameTx) {
    const create = frames.find((f) => f.does.kind === "create");
    const actions = frames.filter((f) => f.mode === "sender" && f.does.kind !== "spend");
    main = create?.does ?? actions.at(-1)?.does ?? { kind: "call", target: null, selector: null };
  } else {
    main = decode("sender", tx.to ?? null, BigInt(tx.value ?? "0x0"), tx.input ?? "0x");
  }

  const gasLimit = frameTx
    ? frames.reduce((sum, f) => sum + f.gasLimit + f.stateLimit, 0n)
    : BigInt(tx.gas ?? "0x0");
  const gasUsed = receipt ? BigInt(receipt.gasUsed) : null;
  const signature = tx.signatures?.[0]?.signature;

  return {
    hash: tx.hash,
    status: !receipt ? "pending" : receipt.status === "0x1" ? "confirmed" : "failed",
    frameTx,
    from,
    payer: receipt?.payer && lower(receipt.payer) !== lower(from) ? receipt.payer : null,
    nonce: tx.nonceSeq ? Number(BigInt(tx.nonceSeq)) : tx.nonce ? Number(BigInt(tx.nonce)) : null,
    block,
    time: header ? new Date(Number(BigInt(header.timestamp)) * 1000) : null,
    gasUsed,
    gasLimit,
    feePaid: receipt && gasUsed !== null ? gasUsed * BigInt(receipt.effectiveGasPrice) : null,
    maxFeePerGas: BigInt(tx.maxFeePerGas ?? tx.gasPrice ?? "0x0"),
    signatureBytes: signature ? (signature.length - 2) / 2 : null,
    main,
    frames,
    tokenMoves: (receipt?.logs ?? [])
      .filter((log) => log.topics[0] === TRANSFER_TOPIC && log.topics.length === 3)
      .map((log) => ({
        token: log.address,
        from: `0x${log.topics[1]!.slice(26)}` as Address,
        to: `0x${log.topics[2]!.slice(26)}` as Address,
        amount: BigInt(log.data),
      })),
  };
}

/** The chain logs ETH movements as Transfer events from this system address (EIP-7708). */
const ETH_LOG_ADDRESS = "0xfffffffffffffffffffffffffffffffffffffffe";

/** The app's token for a contract, if it knows one. ETH's transfer logs count as ETH. */
export function tokenAt(address: Address): Token | undefined {
  if (lower(address) === ETH_LOG_ADDRESS) return KNOWN_TOKENS.find((t) => t.address === null);
  return KNOWN_TOKENS.find((t) => t.address !== null && lower(t.address) === lower(address));
}
