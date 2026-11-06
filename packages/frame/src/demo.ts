// The demo wallet: the same signing calls as a device, with its keys in the page. It's for test ETH only.
// A device has a one-way flash counter and its own screen, and this has neither. What it can do is check
// the chain and browser storage before every signature, and refuse to run anywhere but the test network.
import type { FrameCodec, Hex, NoteSpend, SoftKeys, TxDescription } from "./index";

/** The only chain the demo wallet will sign for. */
export const TEST_CHAIN_ID = 1337;
/** Keys from here on are kept for rotation, as on a device. The demo wallet doesn't rotate. */
export const RESERVED_FROM = 1008;

const SEED_BYTES = 32;
const SPEND_RANDOM_BYTES = 32;
const SEED_KEY = "qday.demo.seed";
const NEXT_KEY = "qday.demo.next";

/** What a person is asked to approve. */
export type DemoRequest = { kind: "transaction"; tx: TxDescription } | ({ kind: "spend" } & NoteSpend);

export interface DemoStorage {
  get(key: string): string | null;
  set(key: string, value: string): void;
  remove(key: string): void;
}

export interface DemoDeps {
  codec: FrameCodec;
  storage: DemoStorage;
  chainId(): Promise<number>;
  /** The account's next one-time key on chain, or 0 if it doesn't exist yet. */
  chainNextIndex(keys: { root: Hex; pubSeed: Hex }): Promise<number>;
  /** Runs `work` with no other tab signing at the same time. */
  lock<T>(work: () => Promise<T>): Promise<T>;
  /** Asks the person, and says whether they approved. */
  approve(request: DemoRequest): Promise<boolean>;
  random(bytes: number): Uint8Array;
}

function hex(bytes: Uint8Array): Hex {
  return `0x${Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("")}`;
}

export class DemoSigner {
  private constructor(
    readonly deps: DemoDeps,
    readonly keys: SoftKeys,
    private readonly seed: Hex,
  ) {}

  /** Opens the wallet kept in this browser, or `seed` if given, or a new one. */
  static async open(deps: DemoDeps, seed?: Hex): Promise<DemoSigner> {
    await requireTestNetwork(deps);
    const stored = deps.storage.get(SEED_KEY);
    const chosen = seed ?? (stored as Hex | null) ?? hex(deps.random(SEED_BYTES));
    const keys = deps.codec.softKeys(chosen);
    if (chosen !== stored) {
      // A local counter belongs to the seed it counted for. The chain has this one's.
      deps.storage.remove(NEXT_KEY);
      deps.storage.set(SEED_KEY, chosen);
      if (deps.storage.get(SEED_KEY) !== chosen) {
        keys.free();
        throw new Error("The demo wallet needs browser storage, and this browser won't keep it.");
      }
    }
    return new DemoSigner(deps, keys, chosen);
  }

  async status() {
    return {
      scheme: "xmss-keccak" as const,
      root: this.keys.root,
      pubSeed: this.keys.pubSeed,
      nextIndex: await this.nextIndex(),
      tree: 0,
      next: null,
    };
  }

  /** Shows the transaction for approval, then signs it with the next one-time key. */
  async signFrame(raw: Uint8Array): Promise<Hex> {
    const { codec, lock, approve } = this.deps;
    const tx = codec.describe(raw);
    const { index } = tx;
    if (index === null) throw new Error("The demo wallet only signs XMSS transactions.");
    if (tx.action.kind === "create" && tx.action.root !== this.keys.root) {
      throw new Error("That account belongs to another key.");
    }
    if (!(await approve({ kind: "transaction", tx }))) throw new Error("rejected");
    return lock(async () => {
      await requireTestNetwork(this.deps);
      const next = await this.nextIndex();
      if (index < next) throw new Error("That one-time key was already used. Review the transaction again.");
      if (index > next) throw new Error("That transaction is out of date. Review it again.");
      if (index >= RESERVED_FROM) throw new Error("This demo wallet is out of keys. Start a new one.");
      this.saveNext(index + 1);
      return this.keys.sign(index, codec.signingMessage(raw));
    });
  }

  async stealthAddress(): Promise<Hex> {
    return this.keys.stealthAddress();
  }

  async viewKey(): Promise<Hex> {
    return this.keys.viewKey();
  }

  /** Shows the spend for approval, then signs it. These keys are one-time by design, so no counter is needed. */
  async spendStealth(spend: NoteSpend): Promise<Hex> {
    if (!(await this.deps.approve({ kind: "spend", ...spend }))) throw new Error("rejected");
    await requireTestNetwork(this.deps);
    return this.keys.spendStealth(spend, this.deps.random(SPEND_RANDOM_BYTES));
  }

  exportSeed(): Hex {
    return this.seed;
  }

  /** Deletes the wallet from this browser. Only the exported seed can bring it back. */
  forget(): void {
    this.deps.storage.remove(SEED_KEY);
    this.deps.storage.remove(NEXT_KEY);
    this.keys.free();
  }

  /** The later of what this browser used and what the chain has seen. A read that fails stops the signature. */
  private async nextIndex(): Promise<number> {
    const local = Number(this.deps.storage.get(NEXT_KEY) ?? 0);
    return Math.max(local, await this.deps.chainNextIndex(this.keys));
  }

  private saveNext(next: number): void {
    this.deps.storage.set(NEXT_KEY, String(next));
    if (this.deps.storage.get(NEXT_KEY) !== String(next)) {
      throw new Error("The demo wallet needs browser storage, and this browser won't keep it.");
    }
  }
}

async function requireTestNetwork(deps: DemoDeps): Promise<void> {
  const id = await deps.chainId();
  if (id !== TEST_CHAIN_ID) throw new Error(`The demo wallet only runs on the test network, and this is chain ${id}.`);
}
