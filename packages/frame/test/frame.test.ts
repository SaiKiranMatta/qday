import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { describe, expect, it } from "vitest";
import { loadFrame } from "../src";

const CLI = `${homedir()}/.cargo/shared-target/release/qday`;
const wasmBytes = readFileSync(new URL("../pkg/qday_wasm_bg.wasm", import.meta.url));
const cli = (...args: string[]) => execFileSync(CLI, args, { encoding: "utf8" }).trim();

const SENDER = "0x8448b66248f3ac3a8ba418512b7c73d8585c5007";
const FACTORY = "0xbcebad3ece45625983409b429406a5dc8d38f876";
const ROOT = `0x${"cd".repeat(32)}` as const;
const SEED = `0x${"96".repeat(32)}` as const;
const TO = "0x8943545177806ed17b9f23f0a21ee5948ecaa776";

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
