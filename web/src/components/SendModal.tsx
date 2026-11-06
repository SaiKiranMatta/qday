import { useState } from "react";
import { formatAmount } from "../lib/chain";
import { isAddress } from "../lib/tokens";
import type { SendStep } from "../lib/useSend";
import { Modal } from "./Modal";
import { StatusDot, Button } from "./ui";

/** Send ETH: enter, review, approve on the device, done. */
export function SendModal({
  balance,
  step,
  onReview,
  onConfirm,
  onBack,
  onClose,
}: {
  balance: string;
  step: SendStep;
  onReview: (to: `0x${string}`, amountEth: string) => void;
  onConfirm: () => void;
  onBack: () => void;
  onClose: () => void;
}) {
  const [to, setTo] = useState("");
  const [amount, setAmount] = useState("");
  const busy = step.kind === "approve" || step.kind === "submitting";
  const validTo = isAddress(to);
  const validAmount = /^\d*\.?\d+$/.test(amount) && Number(amount) > 0;

  return (
    <Modal title="Send ETH" onClose={onClose} locked={busy}>
      {(step.kind === "edit" || step.kind === "error") && (
        <div className="space-y-3">
          <label className="block">
            <span className="text-xs text-muted">To</span>
            <input
              value={to}
              onChange={(e) => setTo(e.target.value.trim())}
              placeholder="0x…"
              className="mt-1 w-full rounded-xl bg-surface-2 px-3 py-2.5 font-mono text-xs outline-none placeholder:text-muted focus:ring-1 focus:ring-slate"
            />
          </label>
          {to && !validTo && <div className="text-xs text-bad">That isn't an address.</div>}
          <label className="block">
            <span className="flex justify-between text-xs text-muted">
              <span>Amount</span>
              <span>Balance {balance} ETH</span>
            </span>
            <div className="mt-1 flex items-center rounded-xl bg-surface-2 px-3 focus-within:ring-1 focus-within:ring-slate">
              <input
                value={amount}
                onChange={(e) => setAmount(e.target.value.trim())}
                placeholder="0.0"
                inputMode="decimal"
                className="min-w-0 flex-1 bg-transparent py-2.5 text-sm outline-none placeholder:text-muted"
              />
              <span className="text-sm text-muted">ETH</span>
            </div>
          </label>
          {step.kind === "error" && (
            <div className="flex items-start gap-2 rounded-xl bg-bad/10 p-3 text-xs text-bad">
              <StatusDot tone="bad" /> <span>{step.message}</span>
            </div>
          )}
          <Button
            className="mt-2 w-full"
            disabled={!validTo || !validAmount}
            onClick={() => validTo && onReview(to as `0x${string}`, amount)}
          >
            Review
          </Button>
        </div>
      )}

      {step.kind === "review" && step.tx.action.kind === "transfer" && (
        <div>
          <div className="text-xs text-muted">You're sending</div>
          <div className="mt-1 text-3xl font-semibold tracking-tight">
            {formatAmount(BigInt(step.tx.action.valueWei), 18)} <span className="text-muted">ETH</span>
          </div>
          <dl className="mt-5 space-y-3 text-sm">
            <div className="flex justify-between gap-4">
              <dt className="text-muted">To</dt>
              <dd className="break-all text-right font-mono text-xs">{step.tx.action.to}</dd>
            </div>
            <div className="flex justify-between gap-4">
              <dt className="text-muted">Network fee</dt>
              <dd>Up to {formatAmount(BigInt(step.tx.maxFeeWei), 18)} ETH</dd>
            </div>
          </dl>
          <p className="mt-5 text-xs text-muted">Your QDay will show the same recipient and amount. Approve only if they match.</p>
          <div className="mt-4 flex gap-3">
            <Button variant="secondary" className="flex-1" onClick={onBack}>
              Back
            </Button>
            <Button className="flex-1" onClick={onConfirm}>
              Send to device
            </Button>
          </div>
        </div>
      )}

      {busy && (
        <div className="py-8 text-center">
          <div className="mx-auto size-8 animate-spin rounded-full border-2 border-line border-t-alabaster" />
          <div className="mt-4 text-sm font-medium">
            {step.kind === "approve" ? "Approve on your QDay" : "Sending…"}
          </div>
          <div className="mt-1 text-xs text-muted">
            {step.kind === "approve" ? "Check the recipient and amount on the device screen." : "Waiting for the next block."}
          </div>
        </div>
      )}

      {step.kind === "done" && (
        <div className="py-6 text-center">
          <div className="mx-auto grid size-12 place-items-center rounded-full bg-ok/15 text-ok">✓</div>
          <div className="mt-4 text-sm font-medium">Sent</div>
          <div className="mt-1 text-xs text-muted">Confirmed in block {step.block}</div>
          <Button className="mt-6 w-full" variant="secondary" onClick={onClose}>
            Done
          </Button>
        </div>
      )}
    </Modal>
  );
}
