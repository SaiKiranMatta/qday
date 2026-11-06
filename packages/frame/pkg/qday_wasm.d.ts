/* tslint:disable */
/* eslint-disable */

/**
 * The account's first transaction, which deploys it through `factory`. Returns the unsigned bytes.
 *
 * # Errors
 *
 * Fails on malformed addresses or keys.
 */
export function createTx(sender: string, index: number, factory: string, root: string, pub_seed: string): Uint8Array;

/**
 * What an unsigned transaction does, as JSON, read back from the bytes the device will see.
 *
 * # Errors
 *
 * Fails unless `raw` is an unsigned `QDay` transaction.
 */
export function describe(raw: Uint8Array | number[]): string;

/**
 * The key blob an ML-DSA account reads, as 0x hex.
 *
 * # Errors
 *
 * Fails on a key of the wrong length, or a degenerate key.
 */
export function mlBlob(public_key: string): string;

/**
 * A stealth payment from an ML-DSA account. Returns the unsigned bytes.
 *
 * # Errors
 *
 * Fails on malformed addresses, amounts or note fields.
 */
export function mlStealthPayTx(sender: string, nonce: bigint, pool: string, value_wei: string, key_hash: string, view_tag: number, sealed: string): Uint8Array;

/**
 * An ETH transfer from an ML-DSA account. `value_wei` is a decimal string. Returns the unsigned bytes.
 *
 * # Errors
 *
 * Fails on malformed addresses or amounts.
 */
export function mlTransferTx(sender: string, nonce: bigint, to: string, value_wei: string): Uint8Array;

/**
 * The hash the device signs for an unsigned transaction, as 0x hex.
 *
 * # Errors
 *
 * Fails unless `raw` is an unsigned `QDay` transaction.
 */
export function signingMessage(raw: Uint8Array | number[]): string;

/**
 * True if an announcement is a payment to the owner of this view seed and address.
 *
 * # Errors
 *
 * Fails on malformed input.
 */
export function stealthFinds(view: string, address: string, key_hash: string, view_tag: number, sealed: string): boolean;

/**
 * The key a spend gives the pool, as 0x hex.
 *
 * # Errors
 *
 * Fails on malformed input.
 */
export function stealthKey(view: string, address: string, sealed: string): string;

/**
 * Makes a payment to a stealth address with 64 random bytes. Returns the note as JSON.
 *
 * # Errors
 *
 * Fails on a malformed address.
 */
export function stealthPay(address: string, random: Uint8Array | number[]): string;

/**
 * A stealth payment from an XMSS account. Returns the unsigned bytes.
 *
 * # Errors
 *
 * Fails on malformed addresses, amounts or note fields.
 */
export function stealthPayTx(sender: string, nonce: bigint, index: number, pool: string, value_wei: string, key_hash: string, view_tag: number, sealed: string): Uint8Array;

/**
 * The `eth_sendFrameTransaction` object for an unsigned transaction and the device's signature, as JSON.
 *
 * # Errors
 *
 * Fails unless `raw` is an unsigned `QDay` transaction.
 */
export function submission(raw: Uint8Array | number[], signature: Uint8Array | number[]): string;

/**
 * An ERC-20 transfer of `amount` (a decimal string, in the token's smallest unit). Returns the unsigned bytes.
 *
 * # Errors
 *
 * Fails on malformed addresses or amounts.
 */
export function tokenTransferTx(sender: string, nonce: bigint, index: number, token: string, to: string, amount: string): Uint8Array;

/**
 * An ETH transfer. `value_wei` is a decimal string. Returns the unsigned bytes.
 *
 * # Errors
 *
 * Fails on malformed addresses or amounts.
 */
export function transferTx(sender: string, nonce: bigint, index: number, to: string, value_wei: string): Uint8Array;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly createTx: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => [number, number, number, number];
    readonly describe: (a: number, b: number) => [number, number, number, number];
    readonly mlBlob: (a: number, b: number) => [number, number, number, number];
    readonly mlStealthPayTx: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => [number, number, number, number];
    readonly mlTransferTx: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number) => [number, number, number, number];
    readonly signingMessage: (a: number, b: number) => [number, number, number, number];
    readonly stealthFinds: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => [number, number, number];
    readonly stealthKey: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly stealthPay: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly stealthPayTx: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number) => [number, number, number, number];
    readonly submission: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly tokenTransferTx: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => [number, number, number, number];
    readonly transferTx: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number, h: number) => [number, number, number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
