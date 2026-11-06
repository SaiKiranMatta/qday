import type { DeviceStatus } from "@qday/device";
import { loadFrame } from "@qday/frame";
import { useCallback, useState } from "react";
import { hexToBytes } from "viem";
import { FACTORY, requestTestEth, submitFrameTx } from "./chain";
import type { Address } from "./tokens";

export type SetupStep =
  | { kind: "idle" }
  | { kind: "funding" }
  | { kind: "approve" }
  | { kind: "submitting" }
  | { kind: "done"; block: number }
  | { kind: "error"; message: string };

function message(error: unknown): string {
  if (error instanceof Error) {
    if (/rejected/.test(error.message)) return "Rejected on the device. Nothing was signed.";
    return error.message;
  }
  return String(error);
}

/**
 * Funding and creating the account. Creation is the account's own first transaction: the browser
 * builds it with the device's frame code, the device signs it, and the account pays for its own
 * deployment from the funds already at its address.
 */
export function useCreateAccount(
  status: DeviceStatus,
  address: Address | null,
  signFrame: (raw: Uint8Array) => Promise<`0x${string}`>,
  onChanged: () => void,
) {
  const [step, setStep] = useState<SetupStep>({ kind: "idle" });

  const fund = useCallback(async () => {
    if (!address) return;
    setStep({ kind: "funding" });
    try {
      await requestTestEth(address);
      setStep({ kind: "idle" });
      onChanged();
    } catch (e) {
      setStep({ kind: "error", message: `Couldn't get test ETH: ${message(e)}` });
    }
  }, [address, onChanged]);

  const create = useCallback(async () => {
    if (!address) return;
    try {
      const frame = await loadFrame();
      const raw = frame.createTx(address, status.nextIndex, FACTORY, status.root, status.pubSeed);
      console.info("[qday] create account", frame.describe(raw), "signs", frame.signingMessage(raw));
      setStep({ kind: "approve" });
      const signature = await signFrame(raw);
      setStep({ kind: "submitting" });
      const receipt = await submitFrameTx(frame.submission(raw, hexToBytes(signature)));
      if (!receipt.ok) throw new Error(`The transaction was mined but failed (block ${receipt.block})`);
      setStep({ kind: "done", block: receipt.block });
      onChanged();
    } catch (e) {
      console.error("[qday] create account failed", e);
      setStep({ kind: "error", message: message(e) });
    }
  }, [address, status, signFrame, onChanged]);

  return { step, fund, create, reset: () => setStep({ kind: "idle" }) };
}
