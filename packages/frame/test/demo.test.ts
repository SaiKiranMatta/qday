import { readFileSync } from "node:fs";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { type DemoDeps, type DemoRequest, DemoSigner, type FrameCodec, RESERVED_FROM, TEST_CHAIN_ID, loadFrame } from "../src";

const wasmBytes = readFileSync(new URL("../pkg/qday_wasm_bg.wasm", import.meta.url));
const SEED = `0x${"5e".repeat(32)}` as const;
const SENDER = `0x${"11".repeat(20)}` as const;
const TO = `0x${"22".repeat(20)}` as const;
const POOL = `0x${"90".repeat(20)}` as const;

let codec: FrameCodec;
beforeAll(async () => {
  codec = await loadFrame(wasmBytes);
});

function memoryStorage() {
  const items = new Map<string, string>();
  return {
    items,
    get: (key: string) => items.get(key) ?? null,
    set: (key: string, value: string) => void items.set(key, value),
    remove: (key: string) => void items.delete(key),
  };
}

/** Runs one at a time, as the browser's lock does. */
function queue() {
  let tail: Promise<unknown> = Promise.resolve();
  return <T>(work: () => Promise<T>): Promise<T> => {
    const run = tail.then(work, work);
    tail = run.catch(() => {});
    return run;
  };
}

function deps(overrides: Partial<DemoDeps> = {}): DemoDeps & { storage: ReturnType<typeof memoryStorage> } {
  return {
    codec,
    storage: memoryStorage(),
    chainId: async () => TEST_CHAIN_ID,
    chainNextIndex: async () => 0,
    lock: queue(),
    approve: async () => true,
    random: (bytes) => new Uint8Array(bytes).fill(7),
    ...overrides,
  } as DemoDeps & { storage: ReturnType<typeof memoryStorage> };
}

const transfer = (index: number) => codec.transferTx(SENDER, 0n, index, TO, 1000n);

describe("the demo wallet's seed", () => {
  it("keeps the same keys across a reload", async () => {
    const shared = deps();
    const first = await DemoSigner.open(shared);
    const second = await DemoSigner.open(shared);
    expect((await second.status()).root).toBe((await first.status()).root);
  });

  it("makes a seed from fresh randomness", async () => {
    const random = vi.fn((bytes: number) => new Uint8Array(bytes).fill(9));
    await DemoSigner.open(deps({ random }));
    expect(random).toHaveBeenCalledWith(32);
  });

  it("restores a pasted seed and exports it again", async () => {
    const signer = await DemoSigner.open(deps(), SEED);
    expect(signer.exportSeed()).toBe(SEED);
  });

  it("refuses to start on a chain that isn't the test network", async () => {
    await expect(DemoSigner.open(deps({ chainId: async () => 1 }))).rejects.toThrow(/test network/);
  });

  it("refuses to start if the browser can't keep the seed", async () => {
    const storage = { get: () => null, set: () => {}, remove: () => {} };
    await expect(DemoSigner.open(deps({ storage }))).rejects.toThrow(/storage/);
  });

  it("forgets the seed and the counter", async () => {
    const shared = deps();
    const signer = await DemoSigner.open(shared);
    await signer.signFrame(transfer(0));
    signer.forget();
    expect(shared.storage.items.size).toBe(0);
  });
});

describe("signing with a one-time key", () => {
  it("gives a whole signature for what was approved", async () => {
    const signer = await DemoSigner.open(deps(), SEED);
    const signature = await signer.signFrame(transfer(0));
    expect((signature.length - 2) / 2).toBe(2500);
  });

  it("moves on to the next key", async () => {
    const signer = await DemoSigner.open(deps(), SEED);
    await signer.signFrame(transfer(0));
    expect((await signer.status()).nextIndex).toBe(1);
  });

  it("never signs the same key twice", async () => {
    const signer = await DemoSigner.open(deps(), SEED);
    await signer.signFrame(transfer(0));
    await expect(signer.signFrame(codec.transferTx(SENDER, 0n, 0, TO, 2000n))).rejects.toThrow(/already used/);
  });

  it("saves the counter before it signs", async () => {
    const shared = deps();
    const signer = await DemoSigner.open(shared, SEED);
    const sign = vi.spyOn(signer.keys, "sign");
    const set = vi.spyOn(shared.storage, "set");
    await signer.signFrame(transfer(0));
    expect(set.mock.invocationCallOrder.at(-1)!).toBeLessThan(sign.mock.invocationCallOrder[0]!);
  });

  it("doesn't sign if the counter can't be saved", async () => {
    const shared = deps();
    const signer = await DemoSigner.open(shared, SEED);
    shared.storage.set = (key, value) => {
      if (!key.includes("next")) shared.storage.items.set(key, value);
    };
    const sign = vi.spyOn(signer.keys, "sign");
    await expect(signer.signFrame(transfer(0))).rejects.toThrow(/storage/);
    expect(sign).not.toHaveBeenCalled();
  });

  it("trusts the chain over an old local counter", async () => {
    const signer = await DemoSigner.open(deps({ chainNextIndex: async () => 7 }), SEED);
    await expect(signer.signFrame(transfer(3))).rejects.toThrow(/already used/);
    await expect(signer.signFrame(transfer(7))).resolves.toMatch(/^0x/);
  });

  it("trusts a local counter ahead of the chain", async () => {
    const signer = await DemoSigner.open(deps(), SEED);
    await signer.signFrame(transfer(0));
    await signer.signFrame(transfer(1));
    expect((await signer.status()).nextIndex).toBe(2);
  });

  it("doesn't sign if the chain can't be read", async () => {
    const signer = await DemoSigner.open(deps(), SEED);
    signer.deps.chainNextIndex = async () => {
      throw new Error("no answer");
    };
    await expect(signer.signFrame(transfer(0))).rejects.toThrow(/no answer/);
  });

  it("refuses a key past the next one, as the device does", async () => {
    const signer = await DemoSigner.open(deps(), SEED);
    await expect(signer.signFrame(transfer(5))).rejects.toThrow(/out of date/);
  });

  it("stops before the keys kept for rotation", async () => {
    const signer = await DemoSigner.open(deps({ chainNextIndex: async () => RESERVED_FROM }), SEED);
    await expect(signer.signFrame(transfer(RESERVED_FROM))).rejects.toThrow(/out of keys/);
  });

  it("refuses to sign on another chain", async () => {
    const signer = await DemoSigner.open(deps(), SEED);
    signer.deps.chainId = async () => 1;
    await expect(signer.signFrame(transfer(0))).rejects.toThrow(/test network/);
  });

  it("refuses an ML-DSA transaction", async () => {
    const signer = await DemoSigner.open(deps(), SEED);
    await expect(signer.signFrame(codec.mlTransferTx(SENDER, 0n, TO, 1000n))).rejects.toThrow(/XMSS/);
  });

  it("won't create an account for another key", async () => {
    const signer = await DemoSigner.open(deps(), SEED);
    const other = `0x${"ab".repeat(32)}` as const;
    await expect(signer.signFrame(codec.createTx(SENDER, 0, TO, other, other))).rejects.toThrow(/another key/);
  });

  it("two tabs asking at once get one signature for a key", async () => {
    const shared = deps();
    const first = await DemoSigner.open(shared, SEED);
    const second = await DemoSigner.open(shared, SEED);
    const results = await Promise.allSettled([first.signFrame(transfer(0)), second.signFrame(transfer(0))]);
    expect(results.map((r) => r.status).sort()).toEqual(["fulfilled", "rejected"]);
  });
});

describe("approval", () => {
  it("shows what the transaction does, read from its bytes", async () => {
    const approve = vi.fn(async (_: DemoRequest) => true);
    const signer = await DemoSigner.open(deps({ approve }), SEED);
    await signer.signFrame(transfer(0));
    expect(approve.mock.calls[0]![0]).toMatchObject({ kind: "transaction", tx: { action: { kind: "transfer", to: TO, valueWei: "1000" } } });
  });

  it("a rejection signs nothing and keeps the key", async () => {
    const signer = await DemoSigner.open(deps({ approve: async () => false }), SEED);
    const sign = vi.spyOn(signer.keys, "sign");
    await expect(signer.signFrame(transfer(0))).rejects.toThrow(/rejected/);
    expect(sign).not.toHaveBeenCalled();
    expect((await signer.status()).nextIndex).toBe(0);
  });
});

describe("private payments", () => {
  it("shares the address and view key without asking", async () => {
    const approve = vi.fn(async () => true);
    const signer = await DemoSigner.open(deps({ approve }), SEED);
    expect(await signer.stealthAddress()).toBe(signer.keys.stealthAddress());
    expect(await signer.viewKey()).toBe(signer.keys.viewKey());
    expect(approve).not.toHaveBeenCalled();
  });

  it("asks before spending, and uses fresh randomness", async () => {
    const approve = vi.fn(async (_: DemoRequest) => true);
    const random = vi.fn((bytes: number) => new Uint8Array(bytes).fill(3));
    const signer = await DemoSigner.open(deps({ approve, random }), SEED);
    const paid = codec.stealthPay(signer.keys.stealthAddress(), new Uint8Array(64).fill(5));
    const spend = { ciphertext: paid.ciphertext, keyHash: paid.keyHash, pool: POOL, to: TO, fee: 10n };
    const signature = await signer.spendStealth(spend);
    expect((signature.length - 2) / 2).toBe(2420);
    expect(approve.mock.calls[0]![0]).toMatchObject({ kind: "spend", to: TO, fee: 10n });
    expect(random).toHaveBeenLastCalledWith(32);
  });

  it("a rejected spend signs nothing", async () => {
    const signer = await DemoSigner.open(deps({ approve: async () => false }), SEED);
    const paid = codec.stealthPay(signer.keys.stealthAddress(), new Uint8Array(64).fill(5));
    const spend = { ciphertext: paid.ciphertext, keyHash: paid.keyHash, pool: POOL, to: TO, fee: 10n };
    await expect(signer.spendStealth(spend)).rejects.toThrow(/rejected/);
  });

  it("refuses to spend on another chain", async () => {
    const signer = await DemoSigner.open(deps(), SEED);
    const paid = codec.stealthPay(signer.keys.stealthAddress(), new Uint8Array(64).fill(5));
    signer.deps.chainId = async () => 1;
    const spend = { ciphertext: paid.ciphertext, keyHash: paid.keyHash, pool: POOL, to: TO, fee: 10n };
    await expect(signer.spendStealth(spend)).rejects.toThrow(/test network/);
  });
});
