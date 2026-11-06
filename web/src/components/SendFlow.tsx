import { type ReactNode, useEffect, useRef, useState } from "react";
import { parseEther } from "viem";
import { addressLines, exactEth, shortHex } from "../lib/format";
import type { Token } from "../lib/tokens";
import { isAddress } from "../lib/tokens";
import type { Draft, SendStep } from "../lib/useSend";
import { TokenLogo } from "./TokenLogo";
import { Button } from "./ui";

/** What the device's review screen prints for the network line. */
const DEVICE_NETWORK = "QDay devnet, chain 1337";

export const SEND_TITLES: Record<SendStep["kind"], string> = {
  edit: "Send",
  review: "Review",
  approve: "Approve on QDay",
  submitting: "Sending",
  done: "Sent",
};

/** The send flow's content: details, then a summary that stays put from review to done. */
export function SendFlow({
  step,
  eth,
  balanceWei,
  feeWei,
  ownAddress,
  onReview,
  onConfirm,
  onEdit,
  onClose,
  initialTo = "",
  initialAmount = "",
}: {
  step: SendStep;
  eth: Token | undefined;
  balanceWei: bigint;
  feeWei: bigint | null;
  ownAddress: string;
  onReview: (to: `0x${string}`, amountEth: string) => void;
  onConfirm: () => void;
  onEdit: () => void;
  onClose: () => void;
  /** Prefill, for the dev gallery. */
  initialTo?: string;
  initialAmount?: string;
}) {
  const [to, setTo] = useState(initialTo);
  const [amount, setAmount] = useState(initialAmount);

  return (
    <div>
      <Progress step={step.kind} />
      {step.kind === "edit" ? (
        <DetailsForm
          to={to}
          amount={amount}
          onTo={setTo}
          onAmount={setAmount}
          eth={eth}
          balanceWei={balanceWei}
          feeWei={feeWei}
          ownAddress={ownAddress}
          error={step.error}
          onReview={onReview}
        />
      ) : (
        <Summary step={step} eth={eth} onConfirm={onConfirm} onEdit={onEdit} onClose={onClose} />
      )}
    </div>
  );
}

const STAGES: SendStep["kind"][][] = [["edit"], ["review", "approve", "submitting"], ["done"]];

/** Three quiet segments: details, confirm, sent. */
function Progress({ step }: { step: SendStep["kind"] }) {
  const at = STAGES.findIndex((stage) => stage.includes(step));
  return (
    <div className="mb-6 flex gap-1.5" role="progressbar" aria-valuemin={1} aria-valuemax={3} aria-valuenow={at + 1}>
      {STAGES.map((_, i) => (
        <span
          key={i}
          className={`h-1 flex-1 rounded-full transition-colors duration-500 ${i <= at ? "bg-alabaster" : "bg-line"}`}
        />
      ))}
    </div>
  );
}

function DetailsForm({
  to,
  amount,
  onTo,
  onAmount,
  eth,
  balanceWei,
  feeWei,
  ownAddress,
  error,
  onReview,
}: {
  to: string;
  amount: string;
  onTo: (v: string) => void;
  onAmount: (v: string) => void;
  eth: Token | undefined;
  balanceWei: bigint;
  feeWei: bigint | null;
  ownAddress: string;
  error?: string;
  onReview: (to: `0x${string}`, amountEth: string) => void;
}) {
  const amountRef = useRef<HTMLInputElement>(null);
  useEffect(() => amountRef.current?.focus(), []);

  const validTo = isAddress(to);
  const toSelf = validTo && to.toLowerCase() === ownAddress.toLowerCase();
  let value: bigint | null = null;
  try {
    value = /^\d*\.?\d+$/.test(amount) ? parseEther(amount) : null;
  } catch {
    value = null;
  }
  const tooMuch = value !== null && feeWei !== null && value + feeWei > balanceWei;
  const maxWei = feeWei !== null && balanceWei > feeWei ? balanceWei - feeWei : 0n;
  const ready = validTo && !toSelf && value !== null && value > 0n && !tooMuch;

  const paste = async () => {
    try {
      onTo((await navigator.clipboard.readText()).trim());
    } catch {
      // Clipboard reading blocked: typing still works.
    }
  };

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (ready) onReview(to as `0x${string}`, amount);
      }}
    >
      <label htmlFor="send-amount" className="flex items-center justify-between text-xs text-muted">
        <span>Amount</span>
        <span className="tabular-nums">Balance {exactEth(balanceWei)} ETH</span>
      </label>
      <div className="mt-2 flex items-center gap-3 rounded-2xl bg-surface-2 px-4 py-3 focus-within:ring-1 focus-within:ring-alabaster/25">
        <input
          id="send-amount"
          ref={amountRef}
          value={amount}
          onChange={(e) => onAmount(e.target.value.replace(",", ".").trim())}
          placeholder="0"
          inputMode="decimal"
          autoComplete="off"
          className="min-w-0 flex-1 bg-transparent text-4xl font-semibold tracking-tight tabular-nums outline-none placeholder:text-muted"
        />
        <span className="flex shrink-0 items-center gap-2 rounded-full bg-surface py-1 pr-3 pl-1 text-sm font-medium">
          <TokenLogo token={eth} size={24} /> ETH
        </span>
      </div>
      <div className="mt-2 flex items-center justify-between text-xs">
        <button
          type="button"
          onClick={() => maxWei > 0n && onAmount(exactEth(maxWei).replace(/,/g, ""))}
          disabled={maxWei === 0n}
          className="rounded-full px-2.5 py-1 font-medium text-alabaster/80 ring-1 ring-line hover:text-alabaster hover:ring-slate disabled:opacity-40"
        >
          Max
        </button>
        <span className={tooMuch ? "text-bad" : "text-muted"}>
          {tooMuch
            ? "More than your balance after the fee"
            : feeWei !== null
              ? `Network fee up to ${exactEth(feeWei)} ETH`
              : "Working out the fee…"}
        </span>
      </div>

      <label htmlFor="send-to" className="mt-6 block text-xs text-muted">
        To
      </label>
      <div className="mt-2 flex items-center gap-2 rounded-2xl bg-surface-2 py-1.5 pr-1.5 pl-4 focus-within:ring-1 focus-within:ring-alabaster/25">
        <input
          id="send-to"
          value={to}
          onChange={(e) => onTo(e.target.value.trim())}
          placeholder="0x…"
          autoComplete="off"
          spellCheck={false}
          className="min-w-0 flex-1 bg-transparent py-1.5 font-mono text-sm outline-none placeholder:text-muted"
        />
        <button
          type="button"
          onClick={paste}
          className="shrink-0 rounded-xl px-3 py-1.5 text-xs font-medium text-alabaster/80 hover:bg-surface hover:text-alabaster"
        >
          Paste
        </button>
      </div>
      <div className="mt-2 min-h-5 font-mono text-xs" aria-live="polite">
        {to && !validTo && <span className="font-sans text-bad">Enter a full 0x address, 42 characters.</span>}
        {toSelf && <span className="font-sans text-warn">That's this account. Send to a different address.</span>}
        {validTo && !toSelf && (
          <span className="text-muted">
            {addressLines(to)[0]} {addressLines(to)[1]}
          </span>
        )}
      </div>

      {error && <Notice tone="bad">{error}</Notice>}

      <Button type="submit" className="mt-5 w-full" disabled={!ready}>
        Review
      </Button>
    </form>
  );
}

/** The transfer as the device will show it. Stays on screen while you approve and while it sends. */
function Summary({
  step,
  eth,
  onConfirm,
  onEdit,
  onClose,
}: {
  step: Exclude<SendStep, { kind: "edit" }>;
  eth: Token | undefined;
  onConfirm: () => void;
  onEdit: () => void;
  onClose: () => void;
}) {
  const confirmRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (step.kind === "review") confirmRef.current?.focus();
  }, [step.kind]);

  const { tx } = step.draft;
  if (tx.action.kind !== "transfer") return null;
  const value = BigInt(tx.action.valueWei);
  const fee = BigInt(tx.maxFeeWei);
  const [line1, line2] = addressLines(tx.action.to);

  if (step.kind === "done") {
    return <Sent draft={step.draft} block={step.block} hash={step.hash} eth={eth} onClose={onClose} />;
  }

  return (
    <div>
      <div className="flex items-center gap-3">
        <TokenLogo token={eth} size={36} />
        <div className="text-4xl font-semibold tracking-tight tabular-nums">
          {exactEth(value)} <span className="text-muted">ETH</span>
        </div>
      </div>

      <dl className="mt-6 divide-y divide-line/60 rounded-2xl bg-surface-2 px-4 text-sm">
        <div className="flex flex-col gap-1 py-3 sm:flex-row sm:gap-4">
          <dt className="w-20 shrink-0 leading-6 text-muted">To</dt>
          {/* Two whole lines, never rewrapped: they have to match the device's lines group for group. */}
          <dd className="font-mono text-[13px] leading-6 whitespace-nowrap tabular-nums">
            {line1}
            <br />
            {line2}
          </dd>
        </div>
        <Row label="Network">{DEVICE_NETWORK}</Row>
        <Row label="Fee">Up to {exactEth(fee)} ETH</Row>
        <Row label="Total" strong>
          Up to {exactEth(value + fee)} ETH
        </Row>
      </dl>

      {step.kind === "review" && (
        <>
          <p className="mt-4 flex items-start gap-2.5 text-xs leading-relaxed text-muted">
            <DeviceIcon className="mt-0.5 size-4 shrink-0 text-alabaster/70" />
            Your QDay shows the same amount and address. Approve on the device only if every group matches.
          </p>
          {step.error && <Notice tone="bad">{step.error}</Notice>}
          <div className="mt-5 flex gap-3">
            <Button variant="secondary" className="px-5" onClick={onEdit}>
              Edit
            </Button>
            <Button ref={confirmRef} className="flex-1 whitespace-nowrap" onClick={onConfirm}>
              {step.error ? "Try again on QDay" : "Continue on QDay"}
            </Button>
          </div>
        </>
      )}

      {(step.kind === "approve" || step.kind === "submitting") && (
        <div className="mt-5 flex items-center gap-4 rounded-2xl bg-surface-2 p-4" role="status" aria-live="polite">
          <span className="relative grid size-11 shrink-0 place-items-center rounded-full bg-onyx">
            {step.kind === "approve" ? (
              <>
                <span className="absolute inset-0 animate-ping rounded-full bg-slate/40 [animation-duration:1.8s]" />
                <DeviceIcon className="relative size-5 text-alabaster" />
              </>
            ) : (
              <span className="size-5 animate-spin rounded-full border-2 border-line border-t-alabaster" />
            )}
          </span>
          <div className="min-w-0">
            <div className="text-sm font-medium">
              {step.kind === "approve" ? "Approve on your QDay" : "Submitting transaction…"}
            </div>
            <div className="mt-0.5 text-xs text-muted">
              {step.kind === "approve"
                ? "Compare the amount and each group of the address, then press Approve. The device waits up to 5 minutes."
                : "Signed on your QDay. This usually takes one block, about 6 seconds."}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function Sent({
  draft,
  block,
  hash,
  eth,
  onClose,
}: {
  draft: Draft;
  block: number;
  hash: string;
  eth: Token | undefined;
  onClose: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const { action } = draft.tx;
  if (action.kind !== "transfer") return null;
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(hash);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard blocked; the hash is selectable.
    }
  };

  return (
    <div role="status">
      <div className="flex flex-col items-center pt-2 text-center">
        <span className="grid size-14 place-items-center rounded-full bg-ok/15 text-ok">
          <svg viewBox="0 0 24 24" className="size-7" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <path d="m5 12.5 4.5 4.5L19 7.5" />
          </svg>
        </span>
        <div className="mt-4 flex items-center gap-2 text-3xl font-semibold tracking-tight tabular-nums">
          <TokenLogo token={eth} size={28} />
          {exactEth(BigInt(action.valueWei))} <span className="text-muted">ETH</span>
        </div>
        <div className="mt-1 text-sm text-muted">sent to {shortHex(action.to, 6, 4)}</div>
      </div>
      <dl className="mt-6 divide-y divide-line/60 rounded-2xl bg-surface-2 px-4 text-sm">
        <Row label="Status">
          <span className="text-ok">Confirmed in block {block.toLocaleString("en-US")}</span>
        </Row>
        <div className="flex items-baseline gap-4 py-3">
          <dt className="w-20 shrink-0 text-muted">Transaction</dt>
          <dd className="flex min-w-0 flex-1 items-baseline gap-3">
            <span className="min-w-0 flex-1 truncate font-mono text-xs" title={hash}>
              {shortHex(hash, 10, 8)}
            </span>
            <button onClick={copy} className="shrink-0 text-xs font-medium text-alabaster/80 hover:text-alabaster">
              {copied ? "Copied" : "Copy"}
            </button>
          </dd>
        </div>
      </dl>
      <Button className="mt-5 w-full" variant="secondary" onClick={onClose}>
        Done
      </Button>
    </div>
  );
}

function Row({ label, strong, children }: { label: string; strong?: boolean; children: ReactNode }) {
  return (
    <div className="flex items-baseline gap-4 py-3">
      <dt className="w-20 shrink-0 text-muted">{label}</dt>
      <dd className={`flex-1 tabular-nums ${strong ? "font-semibold" : ""}`}>{children}</dd>
    </div>
  );
}

function Notice({ tone, children }: { tone: "bad"; children: ReactNode }) {
  return (
    <div role="alert" className={`mt-4 rounded-xl px-3.5 py-3 text-xs leading-relaxed ${tone === "bad" ? "bg-bad/10 text-bad" : ""}`}>
      {children}
    </div>
  );
}

function DeviceIcon({ className = "" }: { className?: string }) {
  return (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" className={className} aria-hidden>
      <rect x="2" y="3.5" width="12" height="9" rx="1.8" />
      <path d="M5 10.5h6" />
    </svg>
  );
}
