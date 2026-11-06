import { type ReactNode, useEffect, useRef, useState } from "react";
import { BLOCK_MS, formatAmount } from "../lib/chain";
import { txUrl } from "../lib/explorer";
import { addressLines, exactEth, exactUnits, parseAmount, shortHex } from "../lib/format";
import { KNOWN_TOKENS, NATIVE_ID, type Token, isAddress } from "../lib/tokens";
import { type Draft, type SendFees, type SendStep, feeFor } from "../lib/useSend";
import { Icon } from "./icons";
import { TokenLogo } from "./TokenLogo";
import { TokenPicker } from "./TokenPicker";
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
  tokens,
  balances,
  fees,
  ownAddress,
  onReview,
  onConfirm,
  onEdit,
  onClose,
  initialTo = "",
  initialAmount = "",
  initialToken = NATIVE_ID,
}: {
  step: SendStep;
  /** Tokens that can be picked. ETH comes first. */
  tokens: Token[];
  /** Exact balance per token id. */
  balances: Record<string, bigint>;
  fees: SendFees | null;
  ownAddress: string;
  onReview: (token: Token, to: `0x${string}`, amount: string) => void;
  onConfirm: () => void;
  onEdit: () => void;
  onClose: () => void;
  /** Prefill, for the dev gallery. */
  initialTo?: string;
  initialAmount?: string;
  initialToken?: string;
}) {
  const [to, setTo] = useState(initialTo);
  const [amount, setAmount] = useState(initialAmount);
  const [tokenId, setTokenId] = useState(initialToken);
  const token = tokens.find((t) => t.id === tokenId) ?? tokens[0] ?? KNOWN_TOKENS[0]!;

  return (
    <div>
      <Progress step={step.kind} />
      {/* Three phases enter once each: details, the summary (review and approve), and the result,
          which stays mounted from sending to sent so its loader can close into the success mark. */}
      <div
        key={step.kind === "edit" ? "edit" : step.kind === "submitting" || step.kind === "done" ? "result" : "summary"}
        className="animate-enter"
      >
        {step.kind === "edit" ? (
          <DetailsForm
            to={to}
            amount={amount}
            token={token}
            tokens={tokens}
            onTo={setTo}
            onAmount={setAmount}
            onToken={(t) => {
              setTokenId(t.id);
              setAmount("");
            }}
            balances={balances}
            fees={fees}
            ownAddress={ownAddress}
            error={step.error}
            onReview={onReview}
          />
        ) : step.kind === "submitting" || step.kind === "done" ? (
          <Result step={step} onClose={onClose} />
        ) : (
          <Summary step={step} onConfirm={onConfirm} onEdit={onEdit} />
        )}
      </div>
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
  token,
  tokens,
  onTo,
  onAmount,
  onToken,
  balances,
  fees,
  ownAddress,
  error,
  onReview,
}: {
  to: string;
  amount: string;
  token: Token;
  tokens: Token[];
  onTo: (v: string) => void;
  onAmount: (v: string) => void;
  onToken: (token: Token) => void;
  balances: Record<string, bigint>;
  fees: SendFees | null;
  ownAddress: string;
  error?: string;
  onReview: (token: Token, to: `0x${string}`, amount: string) => void;
}) {
  const amountRef = useRef<HTMLInputElement>(null);
  useEffect(() => amountRef.current?.focus(), [token.id]);

  const validTo = isAddress(to);
  const toSelf = validTo && to.toLowerCase() === ownAddress.toLowerCase();
  const native = token.address === null;
  const balance = balances[token.id] ?? 0n;
  const ethBalance = balances[NATIVE_ID] ?? 0n;
  const fee = fees ? feeFor(fees, token) : null;
  const value = parseAmount(amount, token.decimals);
  // A well-formed number that has more decimal places than the token.
  const tooPrecise = value === null && /^\d*\.\d+$/.test(amount);
  const tooMuch = value !== null && (native ? fee !== null && value + fee > balance : value > balance);
  const noFee = !native && fee !== null && fee > ethBalance;
  const max = native ? (fee !== null && balance > fee ? balance - fee : 0n) : balance;
  const ready = validTo && !toSelf && value !== null && value > 0n && !tooMuch && !noFee;
  const pickerBalances = Object.fromEntries(tokens.map((t) => [t.id, formatAmount(balances[t.id] ?? 0n, t.decimals)]));

  const note = tooPrecise
    ? { bad: true, text: `${token.symbol} has at most ${token.decimals} decimal places` }
    : tooMuch
      ? { bad: true, text: native ? "More than your balance after the fee" : `More than your ${token.symbol} balance` }
      : noFee
        ? { bad: true, text: "Not enough ETH for the network fee" }
        : { bad: false, text: fee !== null ? `Network fee up to ${exactEth(fee)} ETH` : "Working out the fee…" };

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
        if (ready) onReview(token, to as `0x${string}`, amount);
      }}
    >
      <label htmlFor="send-amount" className="flex items-center justify-between text-xs text-muted">
        <span>Amount</span>
        <span className="tabular-nums">
          Balance {exactUnits(balance, token.decimals)} {token.symbol}
        </span>
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
        <TokenPicker tokens={tokens} selected={token} balances={pickerBalances} onSelect={onToken} />
      </div>
      <div className="mt-2 flex items-center justify-between text-xs">
        <button
          type="button"
          onClick={() => max > 0n && onAmount(exactUnits(max, token.decimals).replace(/,/g, ""))}
          disabled={max === 0n}
          className="rounded-full px-2.5 py-1 font-medium text-alabaster/80 ring-1 ring-line hover:text-alabaster hover:ring-slate disabled:opacity-40"
        >
          Max
        </button>
        <span className={note.bad ? "text-bad" : "text-muted"}>{note.text}</span>
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

/** What a draft sends, read back from the bytes the device will see. */
function sending(draft: Draft) {
  const { action } = draft.tx;
  if (action.kind === "transfer") return { value: BigInt(action.valueWei), to: action.to, contract: null, deviceKnows: true };
  if (action.kind === "token") {
    return { value: BigInt(action.amount), to: action.to, contract: action.token, deviceKnows: action.symbol !== null };
  }
  return null;
}

/** The transfer as the device will show it. Stays on screen while you approve, so the two can be compared. */
function Summary({
  step,
  onConfirm,
  onEdit,
}: {
  step: Extract<SendStep, { kind: "review" | "approve" }>;
  onConfirm: () => void;
  onEdit: () => void;
}) {
  const confirmRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (step.kind === "review") confirmRef.current?.focus();
  }, [step.kind]);

  const { tx, token } = step.draft;
  const send = sending(step.draft);
  if (!send) return null;
  const fee = BigInt(tx.maxFeeWei);

  return (
    <div>
      <div className="flex items-center gap-3">
        <TokenLogo token={token} size={36} />
        <div className="text-4xl font-semibold tracking-tight tabular-nums">
          {exactUnits(send.value, token.decimals)} <span className="text-muted">{token.symbol}</span>
        </div>
      </div>

      {!send.deviceKnows && (
        <Notice tone="warn">
          Your QDay doesn't know this token. It shows the token contract and the amount in the token's smallest
          unit: {send.value.toLocaleString("en-US")}. Check both before you approve.
        </Notice>
      )}

      <dl className="mt-6 divide-y divide-line/60 rounded-2xl bg-surface-2 px-4 text-sm">
        {send.contract && !send.deviceKnows && <AddressRow label="Token" address={send.contract} />}
        <AddressRow label="To" address={send.to} />
        <Row label="Network">{DEVICE_NETWORK}</Row>
        <Row label="Fee">Up to {exactEth(fee)} ETH</Row>
        {!send.contract && (
          <Row label="Total" strong>
            Up to {exactEth(send.value + fee)} ETH
          </Row>
        )}
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

      {step.kind === "approve" && (
        <div className="mt-5 flex items-center gap-3 rounded-2xl bg-surface-2 px-4 py-3.5" role="status" aria-live="polite">
          <span className="relative flex size-2.5 shrink-0">
            <span className="absolute inset-0 animate-ping rounded-full bg-warn/60 [animation-duration:1.6s]" />
            <span className="relative size-2.5 rounded-full bg-warn" />
          </span>
          <span className="text-sm font-medium">Waiting for approval on your QDay</span>
        </div>
      )}
    </div>
  );
}

/** Ring radius, and its circumference (also in the draw keyframes). */
const RING_R = 37;
const RING_LENGTH = 233;

/** The current time, ticking while `active`, for countdowns and elapsed times. */
function useNow(active: boolean, everyMs = 250): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), everyMs);
    return () => clearInterval(timer);
  }, [active, everyMs]);
  return now;
}

const seconds = (ms: number) => `${(Math.max(ms, 0) / 1000).toFixed(1)} s`;

/**
 * Sending, then sent, in one place: the token in a ring that spins while the network confirms and
 * closes into a full circle with a check once it has. Under it, each stage after approval ticks off
 * as it happens, so the wait reads as progress rather than a pause.
 */
function Result({ step, onClose }: { step: Extract<SendStep, { kind: "submitting" | "done" }>; onClose: () => void }) {
  const { token } = step.draft;
  const send = sending(step.draft);
  const done = step.kind === "done";
  const now = useNow(!done);
  if (!send) return null;
  const amount = `${exactUnits(send.value, token.decimals)} ${token.symbol}`;
  const { timing } = step;
  const hash = step.hash;
  const took = done && timing.confirmedAt ? seconds(timing.confirmedAt - timing.signedAt) : null;

  return (
    <div>
      <div className="flex flex-col items-center pt-3 text-center">
        <span className="relative grid size-24 place-items-center">
          <svg viewBox="0 0 80 80" className="absolute inset-0 size-full -rotate-90" aria-hidden>
            <circle cx="40" cy="40" r={RING_R} fill="none" strokeWidth="3" className="stroke-line" />
            {done ? (
              <circle
                cx="40"
                cy="40"
                r={RING_R}
                fill="none"
                strokeWidth="3"
                strokeLinecap="round"
                strokeDasharray={RING_LENGTH}
                className="animate-draw stroke-ok"
              />
            ) : (
              <circle
                cx="40"
                cy="40"
                r={RING_R}
                fill="none"
                strokeWidth="3"
                strokeLinecap="round"
                strokeDasharray={`${RING_LENGTH * 0.28} ${RING_LENGTH}`}
                className="origin-center animate-spin stroke-alabaster [transform-box:fill-box] [animation-duration:1.1s] motion-reduce:animate-none"
              />
            )}
          </svg>
          <TokenLogo token={token} size={56} />
          {done && (
            <span className="absolute right-0.5 bottom-0.5 grid size-7 animate-pop place-items-center rounded-full bg-ok text-onyx ring-4 ring-surface [animation-delay:350ms]">
              <Icon name="check" className="size-4" />
            </span>
          )}
        </span>

        <div className="mt-5 text-2xl font-semibold tracking-tight tabular-nums">{done ? `${amount} sent` : `Sending ${amount}`}</div>
        <div className="mt-1 text-sm text-muted tabular-nums">
          to {shortHex(send.to, 6, 4)}
          {took && <> · confirmed in {took}</>}
        </div>
      </div>

      <ol className="mt-6 space-y-0.5 rounded-2xl bg-surface-2 px-4 py-2 text-sm" aria-live="polite">
        <Stage state="done" label="Signed on your QDay" />
        <Stage
          state={hash ? "done" : "active"}
          label={hash ? "Submitted to the network" : "Submitting to the network"}
          aside={hash ? <TxHash hash={hash} /> : undefined}
        />
        <Stage
          state={done ? "done" : hash ? "active" : "waiting"}
          label={done ? `Confirmed in block ${step.block.toLocaleString("en-US")}` : "Confirming"}
          aside={!done && hash ? <NextBlock at={timing.nextBlockAt} now={now} /> : undefined}
        />
      </ol>

      {done && (
        <div className="mt-5 flex animate-enter gap-3 [animation-delay:200ms]">
          <a
            href={txUrl(step.hash)}
            target="_blank"
            rel="noreferrer"
            className="inline-flex h-11 flex-1 items-center justify-center gap-2 rounded-full bg-surface-2 px-5 text-sm font-semibold transition-[background-color,transform] duration-150 hover:bg-line active:scale-[0.97]"
          >
            View transaction
            <Icon name="external" className="size-4" />
          </a>
          <Button className="flex-1" onClick={onClose}>
            Done
          </Button>
        </div>
      )}
    </div>
  );
}

/** One stage of the send: ticked, in progress, or still to come. */
function Stage({ state, label, aside }: { state: "done" | "active" | "waiting"; label: string; aside?: ReactNode }) {
  return (
    <li className="flex min-h-11 items-center gap-3">
      <span className="grid size-5 shrink-0 place-items-center">
        {state === "done" ? (
          <span className="grid size-5 animate-pop place-items-center rounded-full bg-ok/15 text-ok">
            <Icon name="check" className="size-3" />
          </span>
        ) : state === "active" ? (
          <span className="size-4 animate-spin rounded-full border-2 border-line border-t-alabaster motion-reduce:animate-none" />
        ) : (
          <span className="size-2 rounded-full bg-line" />
        )}
      </span>
      <span className={`min-w-0 flex-1 ${state === "waiting" ? "text-muted" : ""}`}>{label}</span>
      {aside}
    </li>
  );
}

/** The short hash, copyable, as soon as the gateway hands it back. */
function TxHash({ hash }: { hash: string }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(hash);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard blocked; View transaction shows the full hash.
    }
  };
  return (
    <button
      onClick={() => void copy()}
      title={copied ? "Copied" : "Copy transaction hash"}
      className="flex shrink-0 items-center gap-1.5 rounded-lg px-1.5 py-1 font-mono text-xs text-muted transition-colors hover:bg-surface hover:text-alabaster"
    >
      {copied ? <span className="font-sans text-ok">Copied</span> : shortHex(hash, 6, 4)}
      <Icon name={copied ? "check" : "copy"} className="size-3" />
    </button>
  );
}

/** Counts down to the next block slot; past one without a receipt, to the one after. */
function NextBlock({ at, now }: { at: number | undefined; now: number }) {
  if (at === undefined) return <span className="shrink-0 text-xs text-muted">Waiting for a block</span>;
  const left = ((((at - now) % BLOCK_MS) + BLOCK_MS) % BLOCK_MS) / 1000;
  return (
    <span className="shrink-0 text-xs text-muted tabular-nums">
      {left < 0.5 ? "Block due now" : `Next block in ${Math.ceil(left)} s`}
    </span>
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

/** An address in the device's two lines. Never rewrapped: they have to match group for group. */
function AddressRow({ label, address }: { label: string; address: string }) {
  const [line1, line2] = addressLines(address);
  return (
    <div className="flex flex-col gap-1 py-3 sm:flex-row sm:gap-4">
      <dt className="w-20 shrink-0 leading-6 text-muted">{label}</dt>
      <dd className="font-mono text-[13px] leading-6 whitespace-nowrap tabular-nums">
        {line1}
        <br />
        {line2}
      </dd>
    </div>
  );
}

function Notice({ tone, children }: { tone: "bad" | "warn"; children: ReactNode }) {
  return (
    <div role="alert" className={`mt-4 rounded-xl px-3.5 py-3 text-xs leading-relaxed ${tone === "bad" ? "bg-bad/10 text-bad" : "bg-warn/10 text-warn"}`}>
      {children}
    </div>
  );
}

function DeviceIcon({ className = "" }: { className?: string }) {
  return <Icon name="device" className={className} />;
}
