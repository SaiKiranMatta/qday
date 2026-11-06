// The QDAY1 text protocol on the device's USB port. One request, one response.
// Mirrors crates/qday-wallet/src/protocol.rs; keep the two in step.

/** Every protocol line starts with this. Anything else is log output. */
export const PREFIX = "QDAY1 ";
/** The longest line, including the newline. */
export const MAX_LINE = 8192;
/** Length of an encoded XMSS signature. */
export const XMSS_SIG_BYTES = 2500;

export type Hex = `0x${string}`;

export type ErrorCode =
  | "bad-request"
  | "rejected"
  | "keys-exhausted"
  | "counter-failure"
  | "rotation-needed"
  | "nothing-pending"
  | "storage-failure";

const ERROR_CODES: readonly ErrorCode[] = [
  "bad-request",
  "rejected",
  "keys-exhausted",
  "counter-failure",
  "rotation-needed",
  "nothing-pending",
  "storage-failure",
];

/** A prepared next key, as (root, public seed). */
export interface NextKey {
  root: Hex;
  pubSeed: Hex;
}

export type Request =
  | { kind: "pubkey" }
  | { kind: "prepare" }
  | { kind: "signFrame"; raw: Uint8Array }
  | { kind: "activate"; tree: number }
  | { kind: "restore"; seed: Uint8Array; tree: number; chainNextIndex: number };

export type Response =
  | { kind: "pubkey"; root: Hex; pubSeed: Hex; nextIndex: number; tree: number; next: NextKey | null }
  | { kind: "sig"; signature: Hex; next: NextKey | null }
  | { kind: "ok" }
  | { kind: "err"; code: ErrorCode };

/** 0x-prefixed lowercase hex, as crates/qday-wallet/src/hex.rs writes it. */
function toHex(bytes: Uint8Array): Hex {
  return `0x${Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("")}`;
}

/** 0x-prefixed hex of an optional exact byte length, returned lowercase. Null for anything else. */
function readHex(text: string | undefined, bytes?: number): Hex | null {
  if (text === undefined || !text.startsWith("0x")) return null;
  const digits = text.slice(2);
  if (digits.length % 2 !== 0 || !/^[0-9a-fA-F]*$/.test(digits)) return null;
  if (bytes !== undefined && digits.length !== bytes * 2) return null;
  return `0x${digits.toLowerCase()}`;
}

function readNumber(text: string | undefined): number | null {
  if (text === undefined || !/^[0-9]+$/.test(text)) return null;
  const value = Number(text);
  return Number.isSafeInteger(value) && value <= 0xffff_ffff ? value : null;
}

/** Reads an optional next key from the words left on a line. */
function readNext(words: string[]): NextKey | null | undefined {
  if (words.length === 0) return null;
  if (words.length !== 2) return undefined;
  const root = readHex(words[0], 32);
  const pubSeed = readHex(words[1], 32);
  return root && pubSeed ? { root, pubSeed } : undefined;
}

/** Writes a request as a line, ending in a newline. */
export function requestLine(request: Request): string {
  switch (request.kind) {
    case "pubkey":
      return `${PREFIX}PUBKEY\n`;
    case "prepare":
      return `${PREFIX}PREPARE\n`;
    case "signFrame":
      return `${PREFIX}SIGNFRAME ${toHex(request.raw)}\n`;
    case "activate":
      return `${PREFIX}ACTIVATE ${request.tree}\n`;
    case "restore":
      return `${PREFIX}RESTORE ${toHex(request.seed)} ${request.tree} ${request.chainNextIndex}\n`;
  }
}

/** Reads a response line. `null` means it isn't a valid protocol response. */
export function parseResponse(line: string): Response | null {
  const trimmed = line.replace(/\r$/, "");
  if (!trimmed.startsWith(PREFIX)) return null;
  const [word, ...rest] = trimmed.slice(PREFIX.length).split(" ");
  switch (word) {
    case "PUBKEY": {
      const root = readHex(rest[0], 32);
      const pubSeed = readHex(rest[1], 32);
      const nextIndex = readNumber(rest[2]);
      const tree = readNumber(rest[3]);
      const next = readNext(rest.slice(4));
      if (!root || !pubSeed || nextIndex === null || tree === null || next === undefined) return null;
      return { kind: "pubkey", root, pubSeed, nextIndex, tree, next };
    }
    case "SIG": {
      const signature = readHex(rest[0], XMSS_SIG_BYTES);
      const next = readNext(rest.slice(1));
      if (!signature || next === undefined) return null;
      return { kind: "sig", signature, next };
    }
    case "OK":
      return rest.length === 0 ? { kind: "ok" } : null;
    case "ERR": {
      const code = ERROR_CODES.find((known) => known === rest[0]);
      return code && rest.length === 1 ? { kind: "err", code } : null;
    }
    default:
      return null;
  }
}

/** True when a line belongs to the protocol rather than the device's log. */
export function isProtocolLine(line: string): boolean {
  return line.startsWith(PREFIX);
}
