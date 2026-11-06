import type { DeviceStatus } from "@qday/device";
import { type TxDescription, loadFrame } from "@qday/frame";
import { useCallback, useState } from "react";
import { hexToBytes, parseEther } from "viem";
import { submitFrameTx } from "./chain";
import type { Address } from "./tokens";

export type SendStep =
  | { kind: "edit" }
  | { kind: "review"; raw: Uint8Array; tx: TxDescription }
  | { kind: "approve" }
  | { kind: "submitting" }
  | { kind: "done"; block: number; hash: string }
  | { kind: "error"; message: string };

function message(error: unknown): string {
  const text = error instanceof Error ? error.message : String(error);
  if (/rejected/.test(text)) return "Rejected on the device. Nothing was signed.";
  if (/rotation-needed/.test(text)) return "Your QDay needs a key refresh before it can sign more transfers.";
  return text;
}

/** Sending ETH: build with the device's frame code, review, approve on the device, submit. */
export function useSend(
  status: DeviceStatus,
  account: { address: Address; nonce: number; balanceWei: bigint } | null,
  signFrame: (raw: Uint8Array) => Promise<`0x${string}`>,
  onSent: () => void,
) {
  const [step, setStep] = useState<SendStep>({ kind: "edit" });

  /** Builds the transaction and shows it for review. Nothing goes to the device yet. */
  const review = useCallback(
    async (to: Address, amountEth: string) => {
      if (!account) return;
      try {
        const value = parseEther(amountEth);
        if (value <= 0n) throw new Error("Enter an amount above zero.");
        const frame = await loadFrame();
        const raw = frame.transferTx(account.address, BigInt(account.nonce), status.nextIndex, to, value);
        const tx = frame.describe(raw);
        if (value + BigInt(tx.maxFeeWei) > account.balanceWei) {
          throw new Error("Not enough ETH for this amount plus the network fee.");
        }
        setStep({ kind: "review", raw, tx });
      } catch (e) {
        setStep({ kind: "error", message: message(e) });
      }
    },
    [account, status.nextIndex],
  );

  /** Sends the reviewed transaction to the device, then to the network. */
  const confirm = useCallback(async () => {
    if (step.kind !== "review") return;
    const { raw } = step;
    try {
      const frame = await loadFrame();
      console.info("[qday] send", frame.describe(raw), "signs", frame.signingMessage(raw));
      setStep({ kind: "approve" });
      const signature = await signFrame(raw);
      setStep({ kind: "submitting" });
      const receipt = await submitFrameTx(frame.submission(raw, hexToBytes(signature)));
      if (!receipt.ok) throw new Error(`The transaction was mined but failed (block ${receipt.block}).`);
      setStep({ kind: "done", block: receipt.block, hash: receipt.hash });
      onSent();
    } catch (e) {
      console.error("[qday] send failed", e);
      setStep({ kind: "error", message: message(e) });
    }
  }, [step, signFrame, onSent]);

  return { step, review, confirm, reset: () => setStep({ kind: "edit" }) };
}
