import { type ReactNode, useEffect, useState } from "react";
import type { Hex } from "viem";
import { TokenLogo } from "./components/TokenLogo";
import { StatusDot, Wordmark } from "./components/ui";
import { exactEth, exactUnits, shortHex } from "./lib/format";
import { type Action, type FrameDetail, type TxDetail, loadTx, tokenAt } from "./lib/explorer";
import { type Address, KNOWN_TOKENS } from "./lib/tokens";

/** How often to look again while the transaction is unknown or not yet in a block. */
const RETRY_MS = 1_000;
/** After this long without finding it, say so (and keep looking). */
const GIVE_UP_MS = 30_000;

type Load = { kind: "loading" } | { kind: "missing" } | { kind: "error"; message: string } | { kind: "ready"; tx: TxDetail };

/** The built-in explorer: one transaction, read straight from the devnet node. */
export function TxPage({ hash }: { hash: Hex }) {
  const [load, setLoad] = useState<Load>({ kind: "loading" });

  useEffect(() => {
    document.title = `Transaction ${shortHex(hash, 8, 6)} · QDay`;
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const started = Date.now();
    const poll = async () => {
      try {
        const tx = await loadTx(hash);
        if (stopped) return;
        if (tx) setLoad({ kind: "ready", tx });
        else setLoad(Date.now() - started > GIVE_UP_MS ? { kind: "missing" } : { kind: "loading" });
        // Keep watching until the transaction is final.
        if (!tx || tx.status === "pending") timer = setTimeout(() => void poll(), RETRY_MS);
      } catch (e) {
        if (stopped) return;
        setLoad({ kind: "error", message: e instanceof Error ? e.message : String(e) });
        timer = setTimeout(() => void poll(), RETRY_MS * 3);
      }
    };
    void poll();
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  }, [hash]);

  return (
    <div className="min-h-full">
      <header className="mx-auto flex w-full max-w-4xl items-center justify-between gap-4 px-5 py-6 sm:px-8">
        <a href="/" className="flex items-baseline gap-3 rounded-lg" aria-label="QDay wallet">
          <Wordmark className="text-3xl" />
          <span className="text-sm text-muted">Explorer</span>
        </a>
        <a href="/" className="text-sm text-muted transition-colors hover:text-alabaster">
          Open wallet
        </a>
      </header>

      <main className="mx-auto w-full max-w-4xl space-y-6 px-5 pb-16 sm:px-8">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Transaction</h1>
          <div className="mt-2 flex items-start gap-3">
            <code className="min-w-0 font-mono text-xs leading-relaxed break-all text-alabaster/80">{hash}</code>
            <CopyButton text={hash} />
          </div>
        </div>

        {load.kind === "ready" ? (
          <Details tx={load.tx} />
        ) : (
          <section className="rounded-3xl bg-surface px-6 py-10 text-center sm:px-8" role="status" aria-live="polite">
            {load.kind === "loading" && (
              <>
                <span className="mx-auto block size-5 animate-spin rounded-full border-2 border-line border-t-alabaster" />
                <div className="mt-4 text-sm font-medium">Looking for this transaction…</div>
                <div className="mt-1 text-sm text-muted">New transactions show up within a block, about 6 seconds.</div>
              </>
            )}
            {load.kind === "missing" && (
              <>
                <div className="text-sm font-medium">This transaction isn't on the devnet</div>
                <div className="mx-auto mt-1 max-w-md text-sm text-muted">
                  It may not have been submitted, or it belongs to another network. This page keeps checking.
                </div>
              </>
            )}
            {load.kind === "error" && (
              <>
                <div className="text-sm font-medium text-bad">Can't reach the devnet</div>
                <div className="mx-auto mt-1 max-w-md text-sm text-muted">Retrying automatically. {load.message}</div>
              </>
            )}
          </section>
        )}
      </main>
    </div>
  );
}

function Details({ tx }: { tx: TxDetail }) {
  return (
    <>
      <Headline tx={tx} />

      <Card title="Overview">
        <dl className="divide-y divide-line/60">
          <Row label="Status">
            <StatusText status={tx.status} />
          </Row>
          <Row label="Block">{tx.block !== null ? tx.block.toLocaleString("en-US") : "Not in a block yet"}</Row>
          {tx.time && <Row label="Time">{formatTime(tx.time)}</Row>}
          <Row label={tx.frameTx ? "Account" : "From"}>
            <AddressText address={tx.from} />
          </Row>
          {"to" in tx.main && (
            <Row label="To">
              <AddressText address={tx.main.to} />
            </Row>
          )}
          {"token" in tx.main && (
            <Row label="Token">
              <AddressText address={tx.main.token} />
            </Row>
          )}
          {tx.payer && (
            <Row label="Fee paid by">
              <AddressText address={tx.payer} />
            </Row>
          )}
          {tx.nonce !== null && <Row label="Nonce">{tx.nonce.toLocaleString("en-US")}</Row>}
          <Row label="Fee">
            {tx.feePaid !== null ? `${exactEth(tx.feePaid)} ETH` : `Up to ${exactEth(tx.maxFeePerGas * tx.gasLimit)} ETH`}
          </Row>
          <Row label="Gas used">
            {tx.gasUsed !== null ? tx.gasUsed.toLocaleString("en-US") : "—"}
            <span className="text-muted"> of {tx.gasLimit.toLocaleString("en-US")} limit</span>
          </Row>
          <Row label="Type">{tx.frameTx ? "Frame transaction (EIP-8141)" : "Standard transaction"}</Row>
          {tx.signatureBytes !== null && <Row label="Signature">{tx.signatureBytes.toLocaleString("en-US")} bytes</Row>}
        </dl>
      </Card>

      {tx.tokenMoves.length > 0 && (
        <Card title="Transfers">
          <ul className="divide-y divide-line/60">
            {tx.tokenMoves.map((move, i) => {
              const token = tokenAt(move.token);
              return (
                <li key={i} className="flex items-center gap-4 py-3.5">
                  <TokenLogo token={token ?? { symbol: "?" }} size={32} />
                  <div className="min-w-0 flex-1 text-sm">
                    <div className="font-medium tabular-nums">
                      {token ? `${exactUnits(move.amount, token.decimals)} ${token.symbol}` : `${move.amount.toLocaleString("en-US")} units`}
                    </div>
                    <div className="truncate text-xs text-muted">
                      {BigInt(move.from) === 0n ? "Minted" : `From ${shortHex(move.from, 6, 4)}`} to {shortHex(move.to, 6, 4)}
                    </div>
                  </div>
                  {!token && <span className="shrink-0 font-mono text-xs text-muted">{shortHex(move.token, 6, 4)}</span>}
                </li>
              );
            })}
          </ul>
        </Card>
      )}

      {tx.frameTx && (
        <Card title="Frames" note="Steps the transaction runs, in order. All of them are covered by one signature from the device.">
          <ol className="divide-y divide-line/60">
            {tx.frames.map((frame, i) => (
              <FrameRow key={i} index={i} frame={frame} />
            ))}
          </ol>
        </Card>
      )}
    </>
  );
}

/** The point of the transaction in one line, on the brand card. */
function Headline({ tx }: { tx: TxDetail }) {
  const { title, detail, token } = describe(tx.main, tx.from);
  return (
    <section className="chrome-wide grain overflow-hidden rounded-3xl px-6 py-7 sm:px-8 sm:py-8">
      <div className="relative z-10">
        <StatusChip status={tx.status} />
        <div className="mt-4 flex items-center gap-3">
          {token !== undefined && <TokenLogo token={token} size={36} />}
          <div className="min-w-0 text-3xl font-semibold tracking-tight text-balance tabular-nums sm:text-4xl">{title}</div>
        </div>
        {detail && <div className="mt-2 text-sm text-alabaster/75">{detail}</div>}
      </div>
    </section>
  );
}

const ETH = KNOWN_TOKENS.find((t) => t.address === null);

/** Words for an action. `token` is set when an amount of one token moved. */
function describe(action: Action, account: Address): { title: string; detail?: string; token?: { symbol: string; logo?: string } } {
  switch (action.kind) {
    case "send":
      return { title: `${exactEth(action.value)} ETH`, detail: `Sent to ${shortHex(action.to, 6, 4)}`, token: ETH };
    case "token":
    case "mint": {
      const token = tokenAt(action.token);
      const amount = token ? `${exactUnits(action.amount, token.decimals)} ${token.symbol}` : `${action.amount.toLocaleString("en-US")} units`;
      return {
        title: amount,
        detail: `${action.kind === "mint" ? "Minted to" : "Sent to"} ${shortHex(action.to, 6, 4)}`,
        token: token ?? { symbol: "?" },
      };
    }
    case "create":
      return { title: "Account created", detail: `Account ${shortHex(account, 6, 4)} deployed itself, approved on its QDay.` };
    case "rotate":
      return { title: "Keys refreshed", detail: "The account now accepts signatures from the device's new keys." };
    case "verify":
    case "spend":
    case "call":
      return {
        title: "Contract call",
        detail: action.kind === "call" && action.target ? `Called ${shortHex(action.target, 6, 4)}` : undefined,
      };
  }
}

const MODE_LABEL: Record<FrameDetail["mode"], string> = {
  default: "Default",
  verify: "Verify",
  sender: "Sender",
  unknown: "Unknown",
};

function frameWords(frame: FrameDetail): string {
  const does = frame.does;
  switch (does.kind) {
    case "verify":
      return "Checks the device's signature and approves the transaction";
    case "spend":
      return `Uses key #${does.key.toString().padStart(4, "0")}, so it can never sign again`;
    case "rotate":
      return "Switches the account to the device's new keys";
    case "create":
      return "Deploys the account through the account factory";
    case "send":
      return `Sends ${exactEth(does.value)} ETH to ${shortHex(does.to, 6, 4)}`;
    case "token":
    case "mint": {
      const token = tokenAt(does.token);
      const amount = token ? `${exactUnits(does.amount, token.decimals)} ${token.symbol}` : `${does.amount.toLocaleString("en-US")} units`;
      return `${does.kind === "mint" ? "Mints" : "Sends"} ${amount} to ${shortHex(does.to, 6, 4)}`;
    }
    case "call":
      return does.target
        ? `Calls ${shortHex(does.target, 6, 4)}${does.selector ? ` (${does.selector})` : ""}`
        : `Calls the account${does.selector ? ` (${does.selector})` : ""}`;
  }
}

function FrameRow({ index, frame }: { index: number; frame: FrameDetail }) {
  const [open, setOpen] = useState(false);
  const limit = frame.gasLimit + frame.stateLimit;
  return (
    <li className="py-3.5">
      <div className="flex items-start gap-4">
        <span className="grid size-7 shrink-0 place-items-center rounded-full bg-surface-2 text-xs font-semibold tabular-nums">
          {index + 1}
        </span>
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-x-2.5 gap-y-1">
            <span className="rounded-md bg-surface-2 px-1.5 py-0.5 text-[11px] font-medium text-alabaster/80">
              {MODE_LABEL[frame.mode]}
            </span>
            <span className="text-sm">{frameWords(frame)}</span>
          </div>
          <div className="mt-1 text-xs text-muted tabular-nums">
            {frame.gasUsed !== null ? `${frame.gasUsed.toLocaleString("en-US")} gas used of ` : "Gas limit "}
            {limit.toLocaleString("en-US")}
            {frame.ok === false && <span className="text-bad"> · Failed</span>}
          </div>
          {open && (
            <dl className="mt-3 space-y-2 rounded-xl bg-onyx px-4 py-3 text-xs">
              <div className="flex gap-3">
                <dt className="w-14 shrink-0 text-muted">Target</dt>
                <dd className="min-w-0 font-mono break-all">{frame.target ?? "The account itself"}</dd>
              </div>
              {frame.value > 0n && (
                <div className="flex gap-3">
                  <dt className="w-14 shrink-0 text-muted">Value</dt>
                  <dd className="tabular-nums">{exactEth(frame.value)} ETH</dd>
                </div>
              )}
              <div className="flex gap-3">
                <dt className="w-14 shrink-0 text-muted">Data</dt>
                <dd className="min-w-0 font-mono leading-relaxed break-all text-alabaster/80">{frame.data === "0x" ? "None" : frame.data}</dd>
              </div>
            </dl>
          )}
        </div>
        <button
          onClick={() => setOpen((o) => !o)}
          aria-expanded={open}
          className="-mr-2 h-7 shrink-0 rounded-full px-2.5 text-xs font-medium text-muted transition-colors hover:bg-surface-2 hover:text-alabaster"
        >
          {open ? "Hide data" : "Data"}
        </button>
      </div>
    </li>
  );
}

function Card({ title, note, children }: { title: string; note?: string; children: ReactNode }) {
  return (
    <section className="rounded-3xl bg-surface px-6 pt-5 pb-3 sm:px-8 sm:pt-6 sm:pb-4">
      <h2 className="text-base font-semibold">{title}</h2>
      {note && <p className="mt-1 text-xs text-muted">{note}</p>}
      <div className="mt-2">{children}</div>
    </section>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex flex-col gap-1 py-3 text-sm sm:flex-row sm:items-baseline sm:gap-6">
      <dt className="shrink-0 text-muted sm:w-32">{label}</dt>
      <dd className="min-w-0 tabular-nums">{children}</dd>
    </div>
  );
}

const STATUS = {
  confirmed: { text: "Confirmed", tone: "ok", color: "text-ok" },
  failed: { text: "Failed", tone: "bad", color: "text-bad" },
  pending: { text: "Pending", tone: "busy", color: "text-warn" },
} as const;

function StatusChip({ status }: { status: TxDetail["status"] }) {
  const s = STATUS[status];
  return (
    <span className="inline-flex items-center gap-2 rounded-full bg-onyx/70 px-3 py-1 text-xs font-medium backdrop-blur">
      <StatusDot tone={s.tone} /> {s.text}
    </span>
  );
}

function StatusText({ status }: { status: TxDetail["status"] }) {
  const s = STATUS[status];
  return <span className={s.color}>{s.text}</span>;
}

/** A full address, mono, copyable. */
function AddressText({ address }: { address: string }) {
  return (
    <span className="inline-flex max-w-full items-baseline gap-2 align-baseline">
      <span className="min-w-0 font-mono text-xs break-all">{address}</span>
      <CopyButton text={address} small />
    </span>
  );
}

function CopyButton({ text, small = false }: { text: string; small?: boolean }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard blocked; the text is selectable.
    }
  };
  return (
    <button
      onClick={() => void copy()}
      className={`shrink-0 font-sans font-medium text-muted transition-colors hover:text-alabaster ${small ? "text-[11px]" : "text-xs"}`}
    >
      {copied ? "Copied" : "Copy"}
    </button>
  );
}

function formatTime(date: Date): string {
  const seconds = Math.round((Date.now() - date.getTime()) / 1000);
  const ago =
    seconds < 60
      ? `${seconds} s ago`
      : seconds < 3600
        ? `${Math.round(seconds / 60)} min ago`
        : seconds < 86_400
          ? `${Math.round(seconds / 3600)} h ago`
          : `${Math.round(seconds / 86_400)} d ago`;
  const when = date.toLocaleString("en-US", { month: "short", day: "numeric", year: "numeric", hour: "numeric", minute: "2-digit" });
  return `${when} (${ago})`;
}
