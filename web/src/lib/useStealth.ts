import type { Hex, StealthSpend } from "@qday/device";
import { loadFrame } from "@qday/frame";
import { useCallback, useEffect, useRef, useState } from "react";
import { GAS_PRICE, HAS_DEV_KEY, MINE_TIMEOUT_MS, POOL, client, devWallet } from "./chain";
import { SPEND_FEE, SPEND_GAS, SPENT, announcement, poolAbi } from "./stealth";
import type { Address } from "./tokens";

const POLL_MS = 6_000; // one devnet block

/** A payment found in the pool. */
export interface PrivateNote {
  keyHash: Hex;
  ciphertext: Hex;
  valueWei: bigint;
  spent: boolean;
}

export type SetupState = { kind: "off" } | { kind: "sharing" } | { kind: "on" } | { kind: "error"; message: string };

export type ClaimStep =
  | { kind: "edit"; error?: string }
  | { kind: "approve" }
  | { kind: "relaying" }
  | { kind: "done"; hash: Hex; block: number };

interface Keys {
  address: Hex;
  /** Finds payments and can't spend them. Kept in memory only, so each visit asks the device again. */
  view: Hex;
}

interface StealthDevice {
  stealthAddress: () => Promise<Hex>;
  viewKey: () => Promise<Hex>;
  spendStealth: (spend: StealthSpend) => Promise<Hex>;
}

function message(error: unknown): string {
  const text = error instanceof Error ? error.message : String(error);
  if (/rejected/.test(text)) return "Rejected on the device.";
  return text;
}

/** Private payments: the stealth address, the notes found in the pool, and claiming them. */
export function useStealth(device: StealthDevice) {
  const [keys, setKeys] = useState<Keys | null>(null);
  const [setup, setSetup] = useState<SetupState>({ kind: "off" });
  const [notes, setNotes] = useState<PrivateNote[]>([]);
  const [scanned, setScanned] = useState(false);
  const [claim, setClaim] = useState<ClaimStep>({ kind: "edit" });
  // Announcements already checked, so each ciphertext is opened once.
  const found = useRef(new Map<Hex, Hex>());
  const nextBlock = useRef(0n);

  const enable = useCallback(async () => {
    setSetup({ kind: "sharing" });
    try {
      if (!HAS_DEV_KEY) throw new Error("Private payments need a dev key to submit claims, which only the local devnet has.");
      if (!(await client.getCode({ address: POOL }))) {
        throw new Error("The stealth pool isn't on this network. Run tools/devnet/deploy-stealth.sh.");
      }
      const address = await device.stealthAddress();
      const view = await device.viewKey();
      setKeys({ address, view });
      setSetup({ kind: "on" });
    } catch (e) {
      console.error("[qday] private payments setup failed", e);
      setSetup({ kind: "error", message: message(e) });
    }
  }, [device]);

  const scan = useCallback(async () => {
    if (!keys) return;
    const frame = await loadFrame();
    const latest = await client.getBlockNumber();
    if (nextBlock.current <= latest) {
      const logs = await client.getLogs({ address: POOL, event: announcement, fromBlock: nextBlock.current, toBlock: latest });
      for (const log of logs) {
        const { keyHash, viewTag, ciphertext } = log.args;
        if (!keyHash || !viewTag || !ciphertext) continue;
        const note = { keyHash, viewTag: Number.parseInt(viewTag, 16), ciphertext };
        if (frame.stealthFinds(keys.view, keys.address, note)) found.current.set(keyHash, ciphertext);
      }
      nextBlock.current = latest + 1n;
    }
    const entries = [...found.current];
    const balances = await Promise.all(
      entries.map(([keyHash]) => client.readContract({ address: POOL, abi: poolAbi, functionName: "notes", args: [keyHash] })),
    );
    setNotes(
      entries.map(([keyHash, ciphertext], i) => {
        const balance = balances[i] ?? 0n;
        return { keyHash, ciphertext, valueWei: balance === SPENT ? 0n : balance, spent: balance === SPENT };
      }),
    );
    setScanned(true);
  }, [keys]);

  useEffect(() => {
    if (!keys) return;
    const run = () => void scan().catch((e: unknown) => console.warn("[qday] stealth scan failed; will retry", e));
    run();
    const timer = setInterval(run, POLL_MS);
    return () => clearInterval(timer);
  }, [keys, scan]);

  /** Sends a whole note to `to`. The device signs, and the dev key submits and is repaid from the note. */
  const spend = useCallback(
    async (note: PrivateNote, to: Address) => {
      if (!keys) return;
      try {
        const frame = await loadFrame();
        const key = frame.stealthKey(keys.view, keys.address, note.ciphertext);
        setClaim({ kind: "approve" });
        const signature = await device.spendStealth({
          ciphertext: note.ciphertext,
          keyHash: note.keyHash,
          pool: POOL,
          to,
          fee: SPEND_FEE,
        });
        setClaim({ kind: "relaying" });
        const hash = await devWallet().writeContract({
          address: POOL,
          abi: poolAbi,
          functionName: "spend",
          args: [key, signature, to, SPEND_FEE],
          gas: SPEND_GAS * 2n,
          type: "legacy",
          gasPrice: GAS_PRICE,
        });
        const receipt = await client.waitForTransactionReceipt({ hash, timeout: MINE_TIMEOUT_MS });
        if (receipt.status !== "success") throw new Error(`The claim failed in block ${receipt.blockNumber}.`);
        console.info("[qday] claimed", { hash, gasUsed: receipt.gasUsed });
        setClaim({ kind: "done", hash, block: Number(receipt.blockNumber) });
        await scan();
      } catch (e) {
        console.error("[qday] claim failed", e);
        setClaim({ kind: "edit", error: message(e) });
      }
    },
    [keys, device, scan],
  );

  return {
    address: keys?.address ?? null,
    setup,
    notes,
    scanned,
    claim,
    enable,
    spend,
    resetClaim: () => setClaim({ kind: "edit" }),
  };
}
