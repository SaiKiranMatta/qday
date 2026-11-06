import { describe, expect, it } from "vitest";
import { LineBuffer, parseResponse, requestLine } from "../src";

const ROOT = `0x${"ab".repeat(32)}`;
const SEED = `0x${"cd".repeat(32)}`;

describe("requestLine", () => {
  it("writes each request as the Rust side does", () => {
    expect(requestLine({ kind: "pubkey" })).toBe("QDAY1 PUBKEY\n");
    expect(requestLine({ kind: "prepare" })).toBe("QDAY1 PREPARE\n");
    expect(requestLine({ kind: "activate", tree: 3 })).toBe("QDAY1 ACTIVATE 3\n");
    expect(requestLine({ kind: "signFrame", raw: Uint8Array.of(0x06, 0xc0) })).toBe("QDAY1 SIGNFRAME 0x06c0\n");
    expect(requestLine({ kind: "restore", seed: new Uint8Array(32).fill(0xab), tree: 2, chainNextIndex: 164 })).toBe(
      `QDAY1 RESTORE 0x${"ab".repeat(32)} 2 164\n`,
    );
  });
});

describe("parseResponse", () => {
  it("reads PUBKEY with and without a prepared key", () => {
    expect(parseResponse(`QDAY1 PUBKEY ${ROOT} ${SEED} 67 0`)).toEqual({
      kind: "pubkey",
      root: ROOT,
      pubSeed: SEED,
      nextIndex: 67,
      tree: 0,
      next: null,
    });
    expect(parseResponse(`QDAY1 PUBKEY ${ROOT} ${SEED} 1 2 ${SEED} ${ROOT}\r`)).toMatchObject({
      next: { root: SEED, pubSeed: ROOT },
    });
  });

  it("reads SIG, OK and ERR", () => {
    const sig = `0x${"ab".repeat(2500)}`;
    expect(parseResponse(`QDAY1 SIG ${sig}`)).toEqual({ kind: "sig", signature: sig, next: null });
    expect(parseResponse(`QDAY1 PUBKEY ${ROOT.toUpperCase().replace("0X", "0x")} ${SEED} 1 0`)).toMatchObject({ root: ROOT });
    expect(parseResponse("QDAY1 OK")).toEqual({ kind: "ok" });
    expect(parseResponse("QDAY1 ERR rejected")).toEqual({ kind: "err", code: "rejected" });
  });

  it("refuses anything malformed", () => {
    for (const line of [
      "boot: ready",
      "QDAY1 PUBKEY",
      `QDAY1 PUBKEY ${ROOT} ${SEED} -1 0`,
      `QDAY1 PUBKEY ${ROOT.slice(2)} ${SEED} 1 0`,
      `QDAY1 PUBKEY ${ROOT} ${SEED} 1 0 ${SEED}`,
      `QDAY1 SIG 0x${"ab".repeat(10)}`,
      "QDAY1 OK extra",
      "QDAY1 ERR unknown-code",
      "QDAY1 WHAT",
    ]) {
      expect(parseResponse(line), line).toBeNull();
    }
  });
});

describe("LineBuffer", () => {
  it("splits chunks into lines across boundaries", () => {
    const buffer = new LineBuffer();
    const enc = new TextEncoder();
    expect(buffer.push(enc.encode("QDAY1 O"))).toEqual([]);
    expect(buffer.push(enc.encode("K\nlog line\npart"))).toEqual([
      { kind: "text", text: "QDAY1 OK" },
      { kind: "text", text: "log line" },
    ]);
  });

  it("refuses an overlong line and recovers", () => {
    const buffer = new LineBuffer();
    const long = new Uint8Array(9000).fill(0x61);
    expect(buffer.push(long)).toEqual([]);
    expect(buffer.push(new TextEncoder().encode("\nok\n"))).toEqual([{ kind: "bad" }, { kind: "text", text: "ok" }]);
  });
});
