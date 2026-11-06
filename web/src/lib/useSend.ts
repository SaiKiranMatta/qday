import type { DeviceStatus, Hex } from "@qday/device";
import { type FrameCodec, type StealthNote, type TxDescription, loadFrame } from "@qday/frame";
import { useCallback, useEffect, useState } from "react";
import { hexToBytes } from "viem";
import { POOL, submitFrameTx } from "./chain";
import { parseAmount } from "./format";
import { isStealthAddress } from "./stealth";
import { type Address, NATIVE_ID, type Token } from "./tokens";

/** A built transaction, kept on screen from review to done so it can be compared with the device. */
export interface Draft {
  raw: Uint8Array;
  tx: TxDescription;
  /** What is being sent. */
  token: Token;
  /** True for a payment to a stealth address, which shows who it's for nowhere on chain. */
  stealth: boolean;
}

/** The most a send can pay in fees, in wei. ETH and token transfers have different gas limits. */
export interface SendFees {
  eth: bigint;
  token: bigint;
  stealth: bigint;
}

export const feeFor = (fees: SendFees, token: Token, to = "") =>
  token.address ? fees.token : isStealthAddress(to) ? fees.stealth : fees.eth;

/** What a throwaway payment needs: only its size matters, and the bytes never leave this page. */
const SAMPLE_NOTE = { keyHash: `0x${"00".repeat(32)}`, viewTag: 0, ciphertext: `0x${"00".repeat(1120)}` } as const;

/** The three kinds of transaction, for this device's scheme. */
function builders(frame: FrameCodec, status: DeviceStatus, sender: Address, nonce: bigint) {
  if (status.scheme === "ml-dsa-44") {
    return {
      eth: (to: Address, value: bigint) => frame.mlTransferTx(sender, nonce, to, value),
      token: () => {
        throw new Error("ML-DSA accounts can only send ETH so far.");
      },
      stealth: (value: bigint, note: StealthNote) => frame.mlStealthPayTx(sender, nonce, POOL, value, note),
    };
  }
  const index = status.nextIndex;
  return {
    eth: (to: Address, value: bigint) => frame.transferTx(sender, nonce, index, to, value),
    token: (token: Address, to: Address, value: bigint) => frame.tokenTransferTx(sender, nonce, index, token, to, value),
    stealth: (value: bigint, note: StealthNote) => frame.stealthPayTx(sender, nonce, index, POOL, value, note),
  };
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
      const build = builders(frame, status, self, 0n);
      const fee = (raw: Uint8Array) => BigInt(frame.describe(raw).maxFeeWei);
      const eth = fee(build.eth(self, 1n));
      // ML-DSA has no token transfers, so this fee is never shown for it.
      const token = status.scheme === "ml-dsa-44" ? eth : fee(build.token(self, self, 1n));
      setFees({ eth, token, stealth: fee(build.stealth(1n, SAMPLE_NOTE)) });
    });
  }, [account?.address, status.scheme]); // eslint-disable-line react-hooks/exhaustive-deps

  /** Builds the transaction and shows it for review. Nothing goes to the device yet. */
  const review = useCallback(
    async (token: Token, to: Hex, amount: string) => {
      if (!account) return;
      try {
        const value = parseAmount(amount, token.decimals);
        if (value === null || value <= 0n) throw new Error("Enter an amount above zero.");
        const frame = await loadFrame();
        const build = builders(frame, status, account.address, BigInt(account.nonce));
        const stealth = isStealthAddress(to);
        if (stealth && token.address) throw new Error("Private payments can only send ETH.");
        const raw = stealth
          ? build.stealth(value, frame.stealthPay(to))
          : token.address
            ? build.token(token.address, to, value)
            : build.eth(to, value);
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
        setStep({ kind: "review", draft: { raw, tx, token, stealth } });
      } catch (e) {
        setStep({ kind: "edit", error: message(e) });
      }
    },
    [account, status],
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

  return { step, fees, review, confirm, edit: () => setStep({ kind: "edit" }), reset: () => setStep({ kind: "edit" }) };
}
