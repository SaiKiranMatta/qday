import { type ReactNode, useEffect, useRef, useState } from "react";
import { formatAmount } from "../lib/chain";
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
      {/* Details, the summary (review to sending) and the receipt each enter once; the summary stays put while it sends. */}
      <div key={step.kind === "edit" ? "edit" : step.kind === "done" ? "done" : "summary"} className="animate-enter">
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
        ) : (
          <Summary step={step} onConfirm={onConfirm} onEdit={onEdit} onClose={onClose} />
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

/** The transfer as the device will show it. Stays on screen while you approve and while it sends. */
function Summary({
  step,
  onConfirm,
  onEdit,
  onClose,
}: {
  step: Exclude<SendStep, { kind: "edit" }>;
  onConfirm: () => void;
  onEdit: () => void;
  onClose: () => void;
}) {
  const confirmRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (step.kind === "review") confirmRef.current?.focus();
  }, [step.kind]);

  const { tx, token } = step.draft;
  const send = sending(step.draft);
  if (!send) return null;
  const fee = BigInt(tx.maxFeeWei);

  if (step.kind === "done") {
    return <Sent draft={step.draft} block={step.block} hash={step.hash} onClose={onClose} />;
  }

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

function Sent({ draft, block, hash, onClose }: { draft: Draft; block: number; hash: string; onClose: () => void }) {
  const [copied, setCopied] = useState(false);
  const send = sending(draft);
  if (!send) return null;
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
        <span className="grid size-14 animate-pop place-items-center rounded-full bg-ok/15 text-ok">
          <Icon name="check" className="size-7" />
        </span>
        <div className="mt-4 flex items-center gap-2 text-3xl font-semibold tracking-tight tabular-nums">
          <TokenLogo token={draft.token} size={28} />
          {exactUnits(send.value, draft.token.decimals)} <span className="text-muted">{draft.token.symbol}</span>
        </div>
        <div className="mt-1 text-sm text-muted">sent to {shortHex(send.to, 6, 4)}</div>
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
            <a
              href={txUrl(hash)}
              target="_blank"
              rel="noreferrer"
              className="shrink-0 text-xs font-medium text-alabaster/80 hover:text-alabaster"
            >
              View
            </a>
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
