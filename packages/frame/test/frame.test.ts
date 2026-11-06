import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { beforeAll, describe, expect, it } from "vitest";
import { loadFrame } from "../src";

/** The laptop tool, wherever this checkout builds it. */
const CLI_BUILD_MS = 300_000;
const targetDir = () =>
  JSON.parse(execFileSync("cargo", ["metadata", "--format-version", "1", "--no-deps"], { encoding: "utf8" })).target_directory;
let CLI = "";
beforeAll(() => {
  execFileSync("cargo", ["build", "--quiet", "--release", "-p", "qday-cli"], { stdio: "inherit" });
  CLI = `${targetDir()}/release/qday`;
}, CLI_BUILD_MS);

const wasmBytes = readFileSync(new URL("../pkg/qday_wasm_bg.wasm", import.meta.url));
const cli = (...args: string[]) => execFileSync(CLI, args, { encoding: "utf8" }).trim();

const SENDER = "0x8448b66248f3ac3a8ba418512b7c73d8585c5007";
const FACTORY = "0xbcebad3ece45625983409b429406a5dc8d38f876";
const ROOT = `0x${"cd".repeat(32)}` as const;
const SEED = `0x${"96".repeat(32)}` as const;
const TO = "0x8943545177806ed17b9f23f0a21ee5948ecaa776";
const POOL = `0x${"90".repeat(20)}` as const;
const LAPTOP_SEED = `0x${"5e".repeat(32)}` as const;
const RANDOM = new Uint8Array(64).fill(0x55);
const fromHex = (hex: string) => Uint8Array.from(Buffer.from(hex.slice(2), "hex"));

describe("the browser build matches the laptop tool byte for byte", () => {
  it("create: same signing message and submission", async () => {
    const f = await loadFrame(wasmBytes);
    const raw = f.createTx(SENDER, 1, FACTORY, ROOT, SEED);
    expect(f.signingMessage(raw)).toBe(cli("frame-create-hash", SENDER, "1", FACTORY, ROOT, SEED));
    const sig = `0x${"09".repeat(2500)}`;
    expect(f.submission(raw, Buffer.from(sig.slice(2), "hex"))).toEqual(
      JSON.parse(cli("frame-create-json", SENDER, "1", FACTORY, ROOT, SEED, sig)),
    );
    expect(f.describe(raw)).toMatchObject({ index: 1, nonce: 0, action: { kind: "create", factory: FACTORY, root: ROOT } });
  });

  it("transfer: same signing message", async () => {
    const f = await loadFrame(wasmBytes);
    const raw = f.transferTx(SENDER, 2n, 5, TO, 1_000_000_000_000_000n);
    expect(f.signingMessage(raw)).toBe(cli("frame-hash", SENDER, "2", "5", TO, "1000000000000000"));
    expect(f.describe(raw).action).toEqual({ kind: "transfer", to: TO, valueWei: "1000000000000000" });
  });

  it("token transfer: names the token the way the device does", async () => {
    const f = await loadFrame(wasmBytes);
    const usdc = "0xd1108cbf3c5e7b44d7a58d4b2cda492a0ddf6a72";
    const known = f.describe(f.tokenTransferTx(SENDER, 2n, 5, usdc, TO, 125_500_000n)).action;
    expect(known).toEqual({ kind: "token", token: usdc, to: TO, amount: "125500000", symbol: "USDC", decimals: 6 });
    const other = `0x${"5c".repeat(20)}` as const;
    const unknown = f.describe(f.tokenTransferTx(SENDER, 2n, 5, other, TO, 1n)).action;
    expect(unknown).toMatchObject({ kind: "token", token: other, symbol: null, decimals: null });
  });

  it("refuses bytes that aren't a QDay transaction", async () => {
    const f = await loadFrame(wasmBytes);
    expect(() => f.describe(Uint8Array.of(1, 2, 3))).toThrow(/not an unsigned QDay transaction/);
  });
});

describe("ML-DSA accounts in the browser", () => {
  it("transfer: same hash and submission as the laptop tool", async () => {
    const f = await loadFrame(wasmBytes);
    const raw = f.mlTransferTx(SENDER, 2n, TO, 1000n);
    expect(f.signingMessage(raw)).toBe(cli("ml-frame-hash", SENDER, "2", TO, "1000"));
    const sig = `0x${"09".repeat(2420)}`;
    expect(f.submission(raw, fromHex(sig))).toEqual(JSON.parse(cli("ml-frame-json", SENDER, "2", TO, "1000", sig)));
    expect(f.describe(raw)).toMatchObject({ index: null, nonce: 2, action: { kind: "transfer", to: TO, valueWei: "1000" } });
  });

  it("the key blob is the laptop tool's", async () => {
    const f = await loadFrame(wasmBytes);
    const key = cli("mldsa-pubkey", LAPTOP_SEED) as `0x${string}`;
    expect(f.mlBlob(key)).toBe(cli("mldsa-blob", key));
  });

  it("refuses a key of the wrong length", async () => {
    const f = await loadFrame(wasmBytes);
    expect(() => f.mlBlob("0x00")).toThrow(/ML-DSA/);
  });
});

describe("stealth payments in the browser", () => {
  const address = () => cli("stealth-address", LAPTOP_SEED) as `0x${string}`;
  const view = () => cli("stealth-view", LAPTOP_SEED) as `0x${string}`;

  it("a payment is the laptop tool's, byte for byte", async () => {
    const f = await loadFrame(wasmBytes);
    const random = `0x${"55".repeat(64)}`;
    const [keyHash, viewTag, ciphertext] = cli("stealth-pay", address(), random).split(" ");
    expect(f.stealthPay(address(), RANDOM)).toEqual({ keyHash, viewTag: Number(viewTag), ciphertext });
  });

  it("the owner finds a payment and gets the key the pool wants", async () => {
    const f = await loadFrame(wasmBytes);
    const paid = f.stealthPay(address(), RANDOM);
    expect(f.stealthFinds(view(), address(), paid)).toBe(true);
    expect(f.stealthKey(view(), address(), paid.ciphertext)).toBe(cli("stealth-key", view(), address(), paid.ciphertext));
  });

  it("someone else doesn't find it", async () => {
    const f = await loadFrame(wasmBytes);
    const other = `0x${"77".repeat(32)}` as const;
    const otherView = cli("stealth-view", other) as `0x${string}`;
    const otherAddress = cli("stealth-address", other) as `0x${string}`;
    const paid = f.stealthPay(address(), RANDOM);
    expect(f.stealthFinds(otherView, otherAddress, paid)).toBe(false);
  });

  it("a wrong key hash isn't found, even with the right ciphertext", async () => {
    const f = await loadFrame(wasmBytes);
    const paid = f.stealthPay(address(), RANDOM);
    expect(f.stealthFinds(view(), address(), { ...paid, keyHash: `0x${"00".repeat(32)}` })).toBe(false);
  });

  it("each payment is different without a random argument", async () => {
    const f = await loadFrame(wasmBytes);
    expect(f.stealthPay(address()).keyHash).not.toBe(f.stealthPay(address()).keyHash);
  });

  it("refuses an address of the wrong length", async () => {
    const f = await loadFrame(wasmBytes);
    expect(() => f.stealthPay("0x00", RANDOM)).toThrow(/stealth address/);
  });

  it("a payment transaction reads back as one, in both schemes", async () => {
    const f = await loadFrame(wasmBytes);
    const paid = f.stealthPay(address(), RANDOM);
    const action = { kind: "stealthPay", pool: POOL, valueWei: "1000", keyHash: paid.keyHash };
    const xmss = f.stealthPayTx(SENDER, 2n, 5, POOL, 1000n, paid);
    expect(f.describe(xmss)).toMatchObject({ index: 5, action });
    const ml = f.mlStealthPayTx(SENDER, 2n, POOL, 1000n, paid);
    expect(f.describe(ml)).toMatchObject({ index: null, action });
  });
});
