import { describe, expect, it } from "vitest";
import { LineBuffer, MAX_LINE, parseResponse, requestLine } from "../src";

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

describe("requestLine for ML-DSA and stealth", () => {
  it("writes each request as the Rust side does", () => {
    expect(requestLine({ kind: "scheme" })).toBe("QDAY1 SCHEME\n");
    expect(requestLine({ kind: "mlPubkey" })).toBe("QDAY1 MLPUBKEY\n");
    expect(requestLine({ kind: "stealthAddress" })).toBe("QDAY1 STEALTHADDRESS\n");
    expect(requestLine({ kind: "viewKey" })).toBe("QDAY1 VIEWKEY\n");
  });

  it("writes a spend with the fee in decimal", () => {
    const ciphertext = `0x${"c1".repeat(1120)}` as const;
    const keyHash = `0x${"ab".repeat(32)}` as const;
    const pool = `0x${"90".repeat(20)}` as const;
    const to = `0x${"be".repeat(20)}` as const;
    expect(requestLine({ kind: "spendStealth", ciphertext, keyHash, pool, to, fee: 1000n })).toBe(
      `QDAY1 SPENDSTEALTH ${ciphertext} ${keyHash} ${pool} ${to} 1000\n`,
    );
  });

  it("allows the longest line the device reads", () => {
    expect(MAX_LINE).toBe(9216);
  });
});

describe("parseResponse for ML-DSA and stealth", () => {
  it("reads SCHEME by its two names", () => {
    expect(parseResponse("QDAY1 SCHEME xmss")).toEqual({ kind: "scheme", scheme: "xmss-keccak" });
    expect(parseResponse("QDAY1 SCHEME ml-dsa-44")).toEqual({ kind: "scheme", scheme: "ml-dsa-44" });
    expect(parseResponse("QDAY1 SCHEME rsa")).toBeNull();
    expect(parseResponse("QDAY1 SCHEME xmss extra")).toBeNull();
  });

  it("reads keys of exactly the right length", () => {
    const key = `0x${"ab".repeat(1312)}`;
    expect(parseResponse(`QDAY1 MLPUBKEY ${key}`)).toEqual({ kind: "mlPubkey", publicKey: key });
    expect(parseResponse(`QDAY1 MLPUBKEY 0x${"ab".repeat(1311)}`)).toBeNull();
    const address = `0x${"cd".repeat(4160)}`;
    expect(parseResponse(`QDAY1 STEALTHADDRESS ${address}`)).toEqual({ kind: "stealthAddress", address });
    expect(parseResponse(`QDAY1 STEALTHADDRESS 0x${"cd".repeat(4159)}`)).toBeNull();
    const view = `0x${"ef".repeat(32)}`;
    expect(parseResponse(`QDAY1 VIEWKEY ${view}`)).toEqual({ kind: "viewKey", view });
    expect(parseResponse(`QDAY1 VIEWKEY 0x${"ef".repeat(31)}`)).toBeNull();
  });

  it("reads an ML-DSA signature, which never carries a next key", () => {
    const sig = `0x${"ab".repeat(2420)}`;
    expect(parseResponse(`QDAY1 SIG ${sig}`)).toEqual({ kind: "sig", signature: sig, next: null });
    expect(parseResponse(`QDAY1 SIG ${sig} ${ROOT} ${SEED}`)).toBeNull();
  });

  it("still refuses a signature of any other length", () => {
    expect(parseResponse(`QDAY1 SIG 0x${"ab".repeat(2419)}`)).toBeNull();
  });

  it("reads the two new errors", () => {
    expect(parseResponse("QDAY1 ERR wrong-scheme")).toEqual({ kind: "err", code: "wrong-scheme" });
    expect(parseResponse("QDAY1 ERR signing-failed")).toEqual({ kind: "err", code: "signing-failed" });
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
    const long = new Uint8Array(MAX_LINE + 1).fill(0x61);
    expect(buffer.push(long)).toEqual([]);
    expect(buffer.push(new TextEncoder().encode("\nok\n"))).toEqual([{ kind: "bad" }, { kind: "text", text: "ok" }]);
  });
});
