import type { Hex } from "viem";
import { FACTORY, REQUEST_TIMEOUT_MS, RPC_URL, formatAmount } from "./chain";
import { type Address, NATIVE_ID, type Token } from "./tokens";
import type { ActivityItem } from "./wallet";

// There is no indexer on a devnet, so activity comes from scanning blocks. Scans are incremental:
// each poll only reads blocks it hasn't seen.

const BATCH = 100; // blocks per JSON-RPC batch
const MAX_FIRST_SCAN = 20_000; // on a long-running chain, only look this far back the first time

const CREATE_SELECTOR = "0x183815c8";
const ROTATE_SELECTOR = "0xed296e9c";
const TRANSFER_TOPIC = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

interface RpcFrame {
  mode: Hex;
  to: Address | null;
  value: Hex;
  data: Hex;
}

interface RpcTx {
  hash: Hex;
  type: Hex;
  from: Address;
  to?: Address | null;
  sender?: Address;
  value?: Hex;
  frames?: RpcFrame[];
}

interface RpcBlock {
  number: Hex;
  timestamp: Hex;
  transactions: RpcTx[];
}

async function rpc<T>(body: unknown): Promise<T> {
  const res = await fetch(RPC_URL, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
  });
  return (await res.json()) as T;
}

async function call<T>(method: string, params: unknown[]): Promise<T> {
  const body = await rpc<{ result?: T; error?: { message: string } }>({ jsonrpc: "2.0", id: 1, method, params });
  if (body.error) throw new Error(body.error.message);
  return body.result as T;
}

async function blocks(from: number, to: number): Promise<RpcBlock[]> {
  const requests = [];
  for (let n = from; n <= to; n++) {
    requests.push({ jsonrpc: "2.0", id: n, method: "eth_getBlockByNumber", params: [`0x${n.toString(16)}`, true] });
  }
  const replies = await rpc<{ result?: RpcBlock }[]>(requests);
  return replies.map((r) => r.result).filter((b): b is RpcBlock => !!b);
}

function short(address: string): string {
  return `${address.slice(0, 6)}…${address.slice(-4)}`;
}

const lower = (a: string | null | undefined) => (a ?? "").toLowerCase();
const iso = (timestamp: Hex) => new Date(Number(BigInt(timestamp)) * 1000).toISOString();

/** What one frame transaction sent by the account did. */
function ownFrameTx(tx: RpcTx, at: string, eth: Token): ActivityItem | null {
  const frames = tx.frames ?? [];
  const factory = lower(FACTORY);
  if (frames.some((f) => BigInt(f.mode) === 0n && lower(f.to) === factory && f.data.startsWith(CREATE_SELECTOR))) {
    return { id: tx.hash, txHash: tx.hash, kind: "created", at, status: "confirmed" };
  }
  // Frames after the spend frame are the actions; the last one carries the point of the transaction.
  const action = frames.filter((f) => BigInt(f.mode) === 2n).at(-1);
  if (!action) return null;
  if (action.to === null && action.data.startsWith(ROTATE_SELECTOR)) {
    return { id: tx.hash, txHash: tx.hash, kind: "refresh", at, status: "confirmed" };
  }
  if (action.to && BigInt(action.value) > 0n) {
    return {
      id: tx.hash,
      txHash: tx.hash,
      kind: "sent",
      amount: { tokenId: NATIVE_ID, value: formatAmount(BigInt(action.value), eth.decimals) },
      counterparty: short(action.to),
      at,
      status: "confirmed",
    };
  }
  return null;
}

/** ETH that reached the account in this transaction, if any. */
function incomingEth(tx: RpcTx, account: string, at: string, eth: Token): ActivityItem | null {
  if (BigInt(tx.type) === 6n) {
    const paid = (tx.frames ?? []).find((f) => BigInt(f.mode) === 2n && lower(f.to) === account && BigInt(f.value) > 0n);
    if (!paid) return null;
    const from = tx.sender ?? tx.from;
    return {
      id: tx.hash,
      txHash: tx.hash,
      kind: "received",
      amount: { tokenId: NATIVE_ID, value: formatAmount(BigInt(paid.value), eth.decimals) },
      counterparty: short(from),
      at,
      status: "confirmed",
    };
  }
  if (lower(tx.to) !== account || !tx.value || BigInt(tx.value) === 0n) return null;
  return {
    id: tx.hash,
    txHash: tx.hash,
    kind: "received",
    amount: { tokenId: NATIVE_ID, value: formatAmount(BigInt(tx.value), eth.decimals) },
    counterparty: short(tx.from),
    at,
    status: "confirmed",
  };
}

interface Log {
  address: Address;
  topics: Hex[];
  data: Hex;
  blockNumber: Hex;
  transactionHash: Hex;
  logIndex: Hex;
}

async function tokenTransfers(
  account: string,
  from: number,
  to: number,
  tokens: Token[],
  times: Map<number, string>,
): Promise<ActivityItem[]> {
  const erc20 = tokens.filter((t) => t.address);
  if (!erc20.length) return [];
  const topic = `0x${account.slice(2).padStart(64, "0")}`;
  const range = { fromBlock: `0x${from.toString(16)}`, toBlock: `0x${to.toString(16)}`, address: erc20.map((t) => t.address) };
  const [outgoing, incoming] = await Promise.all([
    call<Log[]>("eth_getLogs", [{ ...range, topics: [TRANSFER_TOPIC, topic] }]),
    call<Log[]>("eth_getLogs", [{ ...range, topics: [TRANSFER_TOPIC, null, topic] }]),
  ]);
  const byAddress = new Map(erc20.map((t) => [lower(t.address), t]));
  const item = (log: Log, kind: "sent" | "received"): ActivityItem | null => {
    const token = byAddress.get(lower(log.address));
    if (!token) return null;
    const other = kind === "sent" ? log.topics[2] : log.topics[1];
    const minted = kind === "received" && other !== undefined && BigInt(other) === 0n;
    return {
      id: `${log.transactionHash}:${Number(log.logIndex)}`,
      txHash: log.transactionHash,
      kind,
      amount: { tokenId: token.id, value: formatAmount(BigInt(log.data), token.decimals) },
      counterparty: minted ? "Test mint" : other ? short(`0x${other.slice(26)}`) : undefined,
      at: times.get(Number(log.blockNumber)) ?? new Date().toISOString(),
      status: "confirmed",
    };
  };
  return [
    ...outgoing.map((l) => item(l, "sent")),
    ...incoming.map((l) => item(l, "received")),
  ].filter((x): x is ActivityItem => !!x);
}

/** A running scan of one account's history. */
export class ActivityScanner {
  private next: number | null = null;
  private items: ActivityItem[] = [];

  constructor(
    private readonly account: Address,
    private readonly tokens: Token[],
  ) {}

  /** Reads blocks since the last call and returns the full history, newest first. */
  async update(): Promise<ActivityItem[]> {
    const head = Number(BigInt(await call<Hex>("eth_blockNumber", [])));
    const start = this.next ?? Math.max(0, head - MAX_FIRST_SCAN);
    if (start > head) return this.items;

    const account = lower(this.account);
    const eth = this.tokens.find((t) => t.id === NATIVE_ID);
    const found: ActivityItem[] = [];
    const times = new Map<number, string>();
    const own: Hex[] = [];
    for (let from = start; from <= head; from += BATCH) {
      for (const block of await blocks(from, Math.min(from + BATCH - 1, head))) {
        const at = iso(block.timestamp);
        times.set(Number(block.number), at);
        for (const tx of block.transactions) {
          if (BigInt(tx.type) === 6n && lower(tx.sender ?? tx.from) === account) {
            const mine = eth ? ownFrameTx(tx, at, eth) : null;
            if (mine) {
              found.push(mine);
              own.push(tx.hash);
            }
          } else if (eth) {
            const received = incomingEth(tx, account, at, eth);
            if (received) found.push(received);
          }
        }
      }
    }
    found.push(...(await tokenTransfers(account, start, head, this.tokens, times)));

    // Our own transactions can be mined and still fail; show those as not sent.
    for (const hash of own) {
      const receipt = await call<{ status: Hex } | null>("eth_getTransactionReceipt", [hash]).catch(() => null);
      if (receipt && receipt.status !== "0x1") {
        const item = found.find((f) => f.id === hash);
        if (item) item.status = "failed";
      }
    }

    this.next = head + 1;
    const seen = new Set(this.items.map((i) => i.id));
    this.items = [...this.items, ...found.filter((f) => !seen.has(f.id))].sort((a, b) => b.at.localeCompare(a.at));
    return this.items;
  }
}
