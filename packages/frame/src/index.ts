// The device's own frame code (crates/qday-wasm), compiled to WebAssembly. Every transaction the web
// app sends to the device is built here, so the bytes match what the device and the CLI produce.
import init, * as wasm from "../pkg/qday_wasm.js";

export type Hex = `0x${string}`;

export interface TxDescription {
  sender: Hex;
  nonce: number;
  index: number;
  maxFeeWei: string;
  action:
    | { kind: "transfer"; to: Hex; valueWei: string }
    /** `symbol` and `decimals` are set when the device knows the token, and null when it shows the raw amount. */
    | { kind: "token"; token: Hex; to: Hex; amount: string; symbol: string | null; decimals: number | null }
    | { kind: "rotate"; nextRoot: Hex }
    | { kind: "create"; factory: Hex; root: Hex }
    | { kind: "other"; target: Hex | null; valueWei: string };
}

export interface FrameCodec {
  createTx(sender: Hex, index: number, factory: Hex, root: Hex, pubSeed: Hex): Uint8Array;
  transferTx(sender: Hex, nonce: bigint, index: number, to: Hex, valueWei: bigint): Uint8Array;
  /** An ERC-20 transfer of `amount` (in the token's smallest unit) from the account. */
  tokenTransferTx(sender: Hex, nonce: bigint, index: number, token: Hex, to: Hex, amount: bigint): Uint8Array;
  signingMessage(raw: Uint8Array): Hex;
  /** The eth_sendFrameTransaction object for a signed transaction. */
  submission(raw: Uint8Array, signature: Uint8Array): Record<string, unknown>;
  describe(raw: Uint8Array): TxDescription;
}

let loading: Promise<FrameCodec> | null = null;

function codec(): FrameCodec {
  return {
    createTx: (sender, index, factory, root, pubSeed) => wasm.createTx(sender, index, factory, root, pubSeed),
    transferTx: (sender, nonce, index, to, valueWei) => wasm.transferTx(sender, nonce, index, to, valueWei.toString()),
    tokenTransferTx: (sender, nonce, index, token, to, amount) =>
      wasm.tokenTransferTx(sender, nonce, index, token, to, amount.toString()),
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
