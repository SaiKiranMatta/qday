import { afterEach, describe, expect, it, vi } from "vitest";
import { QDayDevice } from "../src";

const ROOT = `0x${"ab".repeat(32)}`;
const SEED = `0x${"cd".repeat(32)}`;
const ML_KEY = `0x${"ef".repeat(1312)}`;
const CIPHERTEXT = `0x${"c1".repeat(1120)}` as const;
const KEY_HASH = `0x${"ab".repeat(32)}` as const;
const POOL = `0x${"90".repeat(20)}` as const;
const TO = `0x${"be".repeat(20)}` as const;

/** A serial port that answers each request line from a table, and records what it was sent. */
function fakePort(answers: Record<string, string>) {
  const sent: string[] = [];
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  let controller: ReadableStreamDefaultController<Uint8Array>;
  const readable = new ReadableStream<Uint8Array>({ start: (c) => void (controller = c) });
  const writable = new WritableStream<Uint8Array>({
    write(chunk) {
      const line = decoder.decode(chunk).trim();
      sent.push(line);
      const word = line.split(" ")[1] ?? "";
      const answer = answers[word];
      if (answer) controller.enqueue(encoder.encode(`${answer}\n`));
    },
  });
  const port = { open: async () => {}, close: async () => {}, readable, writable } as unknown as SerialPort;
  return { port, sent };
}

afterEach(() => vi.restoreAllMocks());

describe("status", () => {
  it("reads an XMSS device's keys", async () => {
    const { port } = fakePort({ SCHEME: "QDAY1 SCHEME xmss", PUBKEY: `QDAY1 PUBKEY ${ROOT} ${SEED} 3 1` });
    const device = await QDayDevice.open(port);
    expect(await device.status()).toEqual({ scheme: "xmss-keccak", root: ROOT, pubSeed: SEED, nextIndex: 3, tree: 1, next: null });
  });

  it("reads an ML-DSA device's key, and doesn't ask for XMSS keys", async () => {
    const { port, sent } = fakePort({ SCHEME: "QDAY1 SCHEME ml-dsa-44", MLPUBKEY: `QDAY1 MLPUBKEY ${ML_KEY}` });
    const device = await QDayDevice.open(port);
    expect(await device.status()).toEqual({ scheme: "ml-dsa-44", publicKey: ML_KEY });
    expect(sent).not.toContain("QDAY1 PUBKEY");
  });

  it("reports the device's error", async () => {
    const { port } = fakePort({ SCHEME: "QDAY1 ERR storage-failure" });
    const device = await QDayDevice.open(port);
    await expect(device.status()).rejects.toMatchObject({ code: "storage-failure" });
  });
});

describe("stealth", () => {
  it("asks for the address and the view key", async () => {
    const address = `0x${"cd".repeat(4160)}`;
    const view = `0x${"ef".repeat(32)}`;
    const { port } = fakePort({ STEALTHADDRESS: `QDAY1 STEALTHADDRESS ${address}`, VIEWKEY: `QDAY1 VIEWKEY ${view}` });
    const device = await QDayDevice.open(port);
    expect(await device.stealthAddress()).toBe(address);
    expect(await device.viewKey()).toBe(view);
  });

  it("signs a spend", async () => {
    const signature = `0x${"ab".repeat(2420)}`;
    const { port, sent } = fakePort({ SPENDSTEALTH: `QDAY1 SIG ${signature}` });
    const device = await QDayDevice.open(port);
    const spend = { ciphertext: CIPHERTEXT, keyHash: KEY_HASH, pool: POOL, to: TO, fee: 5n };
    expect(await device.spendStealth(spend)).toBe(signature);
    expect(sent[0]).toBe(`QDAY1 SPENDSTEALTH ${CIPHERTEXT} ${KEY_HASH} ${POOL} ${TO} 5`);
  });

  it("passes on a rejection", async () => {
    const { port } = fakePort({ VIEWKEY: "QDAY1 ERR rejected" });
    const device = await QDayDevice.open(port);
    await expect(device.viewKey()).rejects.toMatchObject({ code: "rejected" });
  });
});
