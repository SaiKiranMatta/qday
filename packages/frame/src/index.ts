// The device's own frame code (crates/qday-wasm), compiled to WebAssembly. Every transaction the web
// app sends to the device is built here, so the bytes match what the device and the CLI produce.
import init, * as wasm from "../pkg/qday_wasm.js";

export type Hex = `0x${string}`;

export interface TxDescription {
  sender: Hex;
  nonce: number;
  /** The one-time key this spends. Null for ML-DSA, which has none. */
  index: number | null;
  maxFeeWei: string;
  action:
    | { kind: "transfer"; to: Hex; valueWei: string }
    | { kind: "stealthPay"; pool: Hex; valueWei: string; keyHash: Hex }
    /** `symbol` and `decimals` are set when the device knows the token, and null when it shows the raw amount. */
    | { kind: "token"; token: Hex; to: Hex; amount: string; symbol: string | null; decimals: number | null }
    | { kind: "rotate"; nextRoot: Hex }
    | { kind: "create"; factory: Hex; root: Hex }
    | { kind: "other"; target: Hex | null; valueWei: string };
}

/** What a stealth pool's `pay` call announces. */
export interface StealthNote {
  keyHash: Hex;
  viewTag: number;
  ciphertext: Hex;
}

export interface FrameCodec {
  createTx(sender: Hex, index: number, factory: Hex, root: Hex, pubSeed: Hex): Uint8Array;
  transferTx(sender: Hex, nonce: bigint, index: number, to: Hex, valueWei: bigint): Uint8Array;
  /** An ERC-20 transfer of `amount` (in the token's smallest unit) from the account. */
  tokenTransferTx(sender: Hex, nonce: bigint, index: number, token: Hex, to: Hex, amount: bigint): Uint8Array;
  /** A stealth payment from an XMSS account. */
  stealthPayTx(sender: Hex, nonce: bigint, index: number, pool: Hex, valueWei: bigint, note: StealthNote): Uint8Array;
  mlTransferTx(sender: Hex, nonce: bigint, to: Hex, valueWei: bigint): Uint8Array;
  /** A stealth payment from an ML-DSA account. */
  mlStealthPayTx(sender: Hex, nonce: bigint, pool: Hex, valueWei: bigint, note: StealthNote): Uint8Array;
  /** The key blob an ML-DSA account reads. */
  mlBlob(publicKey: Hex): Hex;
  /** Makes a payment to a stealth address. `random` is for tests: each real payment needs fresh bytes. */
  stealthPay(address: Hex, random?: Uint8Array): StealthNote;
  /** True if an announcement is a payment to this view seed's owner. */
  stealthFinds(view: Hex, address: Hex, note: StealthNote): boolean;
  /** The key a spend gives the pool. */
  stealthKey(view: Hex, address: Hex, ciphertext: Hex): Hex;
  signingMessage(raw: Uint8Array): Hex;
  /** The eth_sendFrameTransaction object for a signed transaction. */
  submission(raw: Uint8Array, signature: Uint8Array): Record<string, unknown>;
  describe(raw: Uint8Array): TxDescription;
}

/** Fresh randomness one stealth payment needs. */
const RANDOM_BYTES = 64;

let loading: Promise<FrameCodec> | null = null;

function codec(): FrameCodec {
  return {
    createTx: (sender, index, factory, root, pubSeed) => wasm.createTx(sender, index, factory, root, pubSeed),
    transferTx: (sender, nonce, index, to, valueWei) => wasm.transferTx(sender, nonce, index, to, valueWei.toString()),
    tokenTransferTx: (sender, nonce, index, token, to, amount) =>
      wasm.tokenTransferTx(sender, nonce, index, token, to, amount.toString()),
    stealthPayTx: (sender, nonce, index, pool, valueWei, note) =>
      wasm.stealthPayTx(sender, nonce, index, pool, valueWei.toString(), note.keyHash, note.viewTag, note.ciphertext),
    mlTransferTx: (sender, nonce, to, valueWei) => wasm.mlTransferTx(sender, nonce, to, valueWei.toString()),
    mlStealthPayTx: (sender, nonce, pool, valueWei, note) =>
      wasm.mlStealthPayTx(sender, nonce, pool, valueWei.toString(), note.keyHash, note.viewTag, note.ciphertext),
    mlBlob: (publicKey) => wasm.mlBlob(publicKey) as Hex,
    stealthPay: (address, random) =>
      JSON.parse(wasm.stealthPay(address, random ?? crypto.getRandomValues(new Uint8Array(RANDOM_BYTES)))) as StealthNote,
    stealthFinds: (view, address, note) => wasm.stealthFinds(view, address, note.keyHash, note.viewTag, note.ciphertext),
    stealthKey: (view, address, ciphertext) => wasm.stealthKey(view, address, ciphertext) as Hex,
    signingMessage: (raw) => wasm.signingMessage(raw) as Hex,
    submission: (raw, signature) => JSON.parse(wasm.submission(raw, signature)) as Record<string, unknown>,
    describe: (raw) => JSON.parse(wasm.describe(raw)) as TxDescription,
  };
}

/** Loads the WebAssembly once. Pass the module bytes outside a browser (tests, Node). */
export function loadFrame(module?: BufferSource): Promise<FrameCodec> {
  loading ??= (module ? init({ module_or_path: module }) : init()).then(codec);
  return loading;
}
