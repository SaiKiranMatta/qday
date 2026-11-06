import type { DeviceStatus } from "@qday/device";
import { type TxDescription, loadFrame } from "@qday/frame";
import { useCallback, useEffect, useState } from "react";
import { hexToBytes, parseEther } from "viem";
import { submitFrameTx } from "./chain";
import type { Address } from "./tokens";

/** A built transaction, kept on screen from review to done so it can be compared with the device. */
export interface Draft {
  raw: Uint8Array;
  tx: TxDescription;
}

export type SendStep =
  | { kind: "edit"; error?: string }
  | { kind: "review"; draft: Draft; error?: string }
  | { kind: "approve"; draft: Draft }
  | { kind: "submitting"; draft: Draft }
  | { kind: "done"; draft: Draft; block: number; hash: string };

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
  const [feeWei, setFeeWei] = useState<bigint | null>(null);

  // The fee limit is fixed by the frame layout, so build one throwaway transfer to learn it.
  useEffect(() => {
    if (!account) return;
    void loadFrame().then((frame) => {
      const probe = frame.transferTx(account.address, 0n, 0, account.address, 1n);
      setFeeWei(BigInt(frame.describe(probe).maxFeeWei));
    });
  }, [account?.address]); // eslint-disable-line react-hooks/exhaustive-deps

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
          throw new Error("Not enough ETH for this amount plus the network fee. Lower the amount, or use Max.");
        }
        setStep({ kind: "review", draft: { raw, tx } });
      } catch (e) {
        setStep({ kind: "edit", error: message(e) });
      }
    },
    [account, status.nextIndex],
  );

  /** Sends the reviewed transaction to the device, then to the network. */
  const confirm = useCallback(async () => {
    if (step.kind !== "review") return;
    const { draft } = step;
    let signed = false;
    try {
      const frame = await loadFrame();
      console.info("[qday] send", draft.tx, "signs", frame.signingMessage(draft.raw));
      setStep({ kind: "approve", draft });
      const signature = await signFrame(draft.raw);
      signed = true;
      setStep({ kind: "submitting", draft });
      const receipt = await submitFrameTx(frame.submission(draft.raw, hexToBytes(signature)));
      if (!receipt.ok) throw new Error(`The transaction was mined but failed (block ${receipt.block}).`);
      setStep({ kind: "done", draft, block: receipt.block, hash: receipt.hash });
      onSent();
    } catch (e) {
      console.error("[qday] send failed", e);
      if (signed) {
        // The device used its key for this draft, so it can't sign it again: rebuild from the form.
        setStep({ kind: "edit", error: `${message(e)} Nothing was sent. Review it again to retry.` });
      } else {
        // Nothing was signed: the same details can go back to the device.
        setStep({ kind: "review", draft, error: message(e) });
      }
    }
  }, [step, signFrame, onSent]);

  return { step, feeWei, review, confirm, edit: () => setStep({ kind: "edit" }), reset: () => setStep({ kind: "edit" }) };
}
