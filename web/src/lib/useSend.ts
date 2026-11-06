import type { DeviceStatus } from "@qday/device";
import { type TxDescription, loadFrame } from "@qday/frame";
import { useCallback, useEffect, useState } from "react";
import { hexToBytes } from "viem";
import { broadcastFrameTx, nextBlockAt, waitForFrameReceipt } from "./chain";
import { parseAmount } from "./format";
import { type Address, NATIVE_ID, type Token } from "./tokens";

/** A built transaction, kept on screen from review to done so it can be compared with the device. */
export interface Draft {
  raw: Uint8Array;
  tx: TxDescription;
  /** What is being sent. */
  token: Token;
}

/** The most a send can pay in fees, in wei. ETH and token transfers have different gas limits. */
export interface SendFees {
  eth: bigint;
  token: bigint;
}

export const feeFor = (fees: SendFees, token: Token) => (token.address ? fees.token : fees.eth);

/** When each stage after approval finished (epoch ms), so the screen can show the send moving. */
export interface SendTiming {
  signedAt: number;
  /** The gateway accepted it. */
  submittedAt?: number;
  /** When the next block is due, while waiting for it. */
  nextBlockAt?: number;
  confirmedAt?: number;
}

export type SendStep =
  | { kind: "edit"; error?: string }
  | { kind: "review"; draft: Draft; error?: string }
  | { kind: "approve"; draft: Draft }
  | { kind: "submitting"; draft: Draft; timing: SendTiming; hash?: string }
  | { kind: "done"; draft: Draft; block: number; hash: string; timing: SendTiming };

function message(error: unknown): string {
  const text = error instanceof Error ? error.message : String(error);
  if (/rejected/.test(text)) return "Rejected on the device. Nothing was signed.";
  if (/rotation-needed/.test(text)) return "Your QDay needs a key refresh before it can sign more transfers.";
  return text;
}

/** Sending ETH or a token: build with the device's frame code, review, approve on the device, submit. */
export function useSend(
  status: DeviceStatus,
  account: { address: Address; nonce: number; units: Record<string, bigint> } | null,
  signFrame: (raw: Uint8Array) => Promise<`0x${string}`>,
  onSent: () => void,
) {
  const [step, setStep] = useState<SendStep>({ kind: "edit" });
  const [fees, setFees] = useState<SendFees | null>(null);

  // Fee limits are fixed by the frame layout, so build one throwaway transaction of each kind to learn them.
  useEffect(() => {
    if (!account) return;
    void loadFrame().then((frame) => {
      const self = account.address;
      const eth = frame.describe(frame.transferTx(self, 0n, 0, self, 1n)).maxFeeWei;
      const token = frame.describe(frame.tokenTransferTx(self, 0n, 0, self, self, 1n)).maxFeeWei;
      setFees({ eth: BigInt(eth), token: BigInt(token) });
    });
  }, [account?.address]); // eslint-disable-line react-hooks/exhaustive-deps

  /** Builds the transaction and shows it for review. Nothing goes to the device yet. */
  const review = useCallback(
    async (token: Token, to: Address, amount: string) => {
      if (!account) return;
      try {
        const value = parseAmount(amount, token.decimals);
        if (value === null || value <= 0n) throw new Error("Enter an amount above zero.");
        const frame = await loadFrame();
        const nonce = BigInt(account.nonce);
        const raw = token.address
          ? frame.tokenTransferTx(account.address, nonce, status.nextIndex, token.address, to, value)
          : frame.transferTx(account.address, nonce, status.nextIndex, to, value);
        const tx = frame.describe(raw);
        const fee = BigInt(tx.maxFeeWei);
        const eth = account.units[NATIVE_ID] ?? 0n;
        if (!token.address && value + fee > eth) {
          throw new Error("Not enough ETH for this amount plus the network fee. Lower the amount, or use Max.");
        }
        if (token.address && value > (account.units[token.id] ?? 0n)) {
          throw new Error(`Not enough ${token.symbol}. Lower the amount, or use Max.`);
        }
        if (token.address && fee > eth) throw new Error("Not enough ETH to pay the network fee.");
        setStep({ kind: "review", draft: { raw, tx, token } });
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
      // Each stage shows as it finishes: signed, accepted by the gateway, then mined.
      const timing: SendTiming = { signedAt: Date.now() };
      setStep({ kind: "submitting", draft, timing });
      const hash = await broadcastFrameTx(frame.submission(draft.raw, hexToBytes(signature)));
      timing.submittedAt = Date.now();
      timing.nextBlockAt = await nextBlockAt().catch(() => undefined);
      setStep({ kind: "submitting", draft, timing: { ...timing }, hash });
      const receipt = await waitForFrameReceipt(hash);
      if (!receipt.ok) throw new Error(`The transaction was mined but failed (block ${receipt.block}).`);
      setStep({ kind: "done", draft, block: receipt.block, hash: receipt.hash, timing: { ...timing, confirmedAt: Date.now() } });
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

  return { step, fees, review, confirm, edit: () => setStep({ kind: "edit" }), reset: () => setStep({ kind: "edit" }) };
}
