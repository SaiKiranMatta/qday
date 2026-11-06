import type { DeviceStatus } from "@qday/device";
import { type ComponentProps, type ReactNode, useState } from "react";
import { ActivityList, ActivitySkeleton } from "./components/ActivityList";
import { ManageTokens } from "./components/ManageTokens";
import { TokenLogo } from "./components/TokenLogo";
import { Button, StatusDot } from "./components/ui";
import { capacity } from "./lib/capacity";
import { groupThousands, shortHex } from "./lib/format";
import { SetupCard } from "./components/SetupCard";
import type { ChainState } from "./lib/useAccount";
import type { SetupStep } from "./lib/useCreateAccount";
import type { TokenList } from "./lib/useTokens";
import type { AccountView } from "./lib/wallet";

/** Balance, assets and recent activity. */
export function WalletPage({
  account,
  tokens,
  chain,
  error,
  setup,
  onSend,
  onReceive,
  onMintTokens,
  onOpenActivity,
}: {
  account: AccountView;
  tokens: TokenList;
  chain: ChainState;
  error: string | null;
  /** Present until the account exists. */
  setup: {
    address: `0x${string}` | null;
    balanceWei: bigint;
    step: SetupStep;
    onFund: () => void;
    onCreate: () => void;
  } | null;
  /** Null while sending isn't possible (no account yet, or no device in mock mode). */
  onSend: (() => void) | null;
  onReceive: (() => void) | null;
  /** Local devnet only: mints test tokens to the account. */
  onMintTokens: (() => Promise<void>) | null;
  onOpenActivity: () => void;
}) {
  const [managing, setManaging] = useState(false);
  const [minting, setMinting] = useState<"idle" | "busy" | "error">("idle");
  const mint = async () => {
    if (!onMintTokens) return;
    setMinting("busy");
    try {
      await onMintTokens();
      setMinting("idle");
    } catch (e) {
      console.error("[qday] minting test tokens failed", e);
      setMinting("error");
    }
  };
  return (
    <div className="space-y-6">
      <section className="chrome-wide grain overflow-hidden rounded-3xl px-6 py-7 sm:px-8 sm:py-8">
        <div className="relative z-10 flex flex-wrap items-end justify-between gap-6">
          {setup ? (
            <SetupCard {...setup} />
          ) : (
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <span className="text-sm font-medium text-alabaster/90">{account.name}</span>
                {account.address !== "0x" && <CopyAddress address={account.address} />}
              </div>
              <div className="mt-3 text-5xl font-semibold tracking-tight tabular-nums">
                {chain === "loading" ? "…" : account.balance} <span className="text-alabaster/60">ETH</span>
              </div>
            </div>
          )}
          {!setup && (
            <div className="flex gap-3">
              <Button variant="onyx" disabled={!onSend} onClick={() => onSend?.()}>
                Send
              </Button>
              <Button variant="onyx" disabled={!onReceive} onClick={() => onReceive?.()}>
                Receive
              </Button>
            </div>
          )}
        </div>
      </section>

      {chain === "error" && (
        <div role="alert" className="rounded-2xl bg-bad/10 px-5 py-4 text-sm text-bad">
          Can't reach the devnet, so balances may be out of date. Retrying automatically.
          {error && <span className="mt-1 block text-xs text-bad/80">{error}</span>}
        </div>
      )}

      <div className="grid gap-6 xl:grid-cols-2 xl:items-start">
        <Card
          title="Assets"
          actions={
            <>
              {onMintTokens && (
                <CardAction onClick={() => void mint()} disabled={minting === "busy"} tone={minting === "error" ? "bad" : "muted"}>
                  {minting === "busy" ? "Getting tokens…" : minting === "error" ? "Couldn't get tokens. Retry" : "Get test tokens"}
                </CardAction>
              )}
              <CardAction onClick={() => setManaging(true)}>Manage</CardAction>
            </>
          }
        >
          <ul className="divide-y divide-line/60">
            {tokens.selected.map((token) => (
              <li key={token.id} className="flex items-center gap-4 py-3.5">
                <TokenLogo token={token} />
                <div className="min-w-0 flex-1">
                  <div className="truncate text-sm font-medium">{token.name}</div>
                  <div className="text-xs text-muted">{token.symbol}</div>
                </div>
                <div className="text-right text-sm font-medium tabular-nums">
                  {account.balances[token.id] ?? "0"} <span className="text-muted">{token.symbol}</span>
                </div>
              </li>
            ))}
          </ul>
        </Card>

        <Card
          title="Recent activity"
          actions={account.activity.length > 0 && <CardAction onClick={onOpenActivity}>View all</CardAction>}
        >
          {!account.activityReady ? (
            <ActivitySkeleton />
          ) : account.activity.length ? (
            <ActivityList items={account.activity} tokens={tokens.byId} limit={5} />
          ) : (
            <EmptyState title="No activity yet" body="Transfers in and out of this account will show up here." />
          )}
        </Card>
      </div>

      {managing && <ManageTokens tokens={tokens} onClose={() => setManaging(false)} />}
    </div>
  );
}

/** A content card with a title row. Every card shares this padding, so text lines up down the page. */
function Card({ title, actions, children }: { title: string; actions?: ReactNode; children: ReactNode }) {
  return (
    <section className="rounded-3xl bg-surface px-6 pt-5 pb-3 sm:px-8 sm:pt-6 sm:pb-4">
      <div className="flex min-h-8 items-center justify-between gap-4">
        <h2 className="text-base font-semibold">{title}</h2>
        {actions && <div className="-mr-3 flex items-center gap-1">{actions}</div>}
      </div>
      <div className="mt-2">{children}</div>
    </section>
  );
}

/** A quiet text action in a card's title row, with a real hit area. */
function CardAction({
  tone = "muted",
  className = "",
  ...props
}: ComponentProps<"button"> & { tone?: "muted" | "bad" }) {
  return (
    <button
      className={`h-8 rounded-full px-3 text-xs font-medium transition-colors hover:bg-surface-2 hover:text-alabaster disabled:pointer-events-none disabled:opacity-60 ${
        tone === "bad" ? "text-bad" : "text-muted"
      } ${className}`}
      {...props}
    />
  );
}

/** The short address; a click copies the whole of it. */
function CopyAddress({ address }: { address: string }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(address);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard blocked; Receive shows the full address.
    }
  };
  return (
    <button
      onClick={() => void copy()}
      title={address}
      aria-label="Copy address"
      className="rounded-full bg-onyx/40 px-2.5 py-1 font-mono text-xs text-alabaster/70 transition-colors hover:bg-onyx/60 hover:text-alabaster"
    >
      {copied ? "Copied" : shortHex(address, 6, 4)}
    </button>
  );
}

type ActivityFilter = "all" | "sent" | "received";

const FILTERS: { id: ActivityFilter; label: string; empty: string }[] = [
  { id: "all", label: "All", empty: "Transfers in and out of this account will show up here." },
  { id: "sent", label: "Sent", empty: "Transfers you approve on your QDay will show up here." },
  { id: "received", label: "Received", empty: "Funds sent to this account will show up here." },
];

const PAGE_SIZE = 25;

export function ActivityPage({ account, tokens }: { account: AccountView; tokens: TokenList }) {
  const [filter, setFilter] = useState<ActivityFilter>("all");
  const [shown, setShown] = useState(PAGE_SIZE);
  const items = account.activity.filter((item) =>
    filter === "all" ? true : filter === "sent" ? item.kind === "sent" || item.kind === "refused" : item.kind === "received",
  );
  const active = FILTERS.find((f) => f.id === filter)!;

  return (
    <div className="max-w-3xl">
      <div className="mb-6 flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">Activity</h1>
          <p className="mt-1 text-sm text-muted">Transfers and account changes, newest first.</p>
        </div>
        <div role="tablist" aria-label="Show" className="flex rounded-full bg-surface p-1">
          {FILTERS.map((f) => (
            <button
              key={f.id}
              role="tab"
              aria-selected={filter === f.id}
              onClick={() => {
                setFilter(f.id);
                setShown(PAGE_SIZE);
              }}
              className={`h-8 rounded-full px-4 text-xs font-medium transition-colors ${
                filter === f.id ? "bg-surface-2 text-alabaster" : "text-muted hover:text-alabaster"
              }`}
            >
              {f.label}
            </button>
          ))}
        </div>
      </div>

      <section className="rounded-3xl bg-surface px-6 py-3 sm:px-8 sm:py-4">
        {!account.activityReady ? (
          <ActivitySkeleton rows={6} />
        ) : items.length ? (
          <>
            <ActivityList items={items} tokens={tokens.byId} limit={shown} />
            {items.length > shown && (
              <div className="border-t border-line/60 py-3 text-center">
                <button
                  onClick={() => setShown((n) => n + PAGE_SIZE)}
                  className="h-8 rounded-full px-4 text-xs font-medium text-muted transition-colors hover:bg-surface-2 hover:text-alabaster"
                >
                  Show more
                </button>
              </div>
            )}
          </>
        ) : (
          <EmptyState title={filter === "all" ? "No activity yet" : `Nothing ${active.label.toLowerCase()} yet`} body={active.empty} />
        )}
      </section>

      {account.activityReady && items.some((item) => item.txHash) && (
        <p className="mt-4 text-center text-xs text-muted">Select a transaction to open it in the explorer.</p>
      )}
    </div>
  );
}

export function DevicePage({
  status,
  log,
  onRefresh,
  onDisconnect,
}: {
  status: DeviceStatus;
  log: string[];
  onRefresh: () => void;
  onDisconnect: () => void;
}) {
  const [showDetails, setShowDetails] = useState(false);
  const cap = capacity(status);
  return (
    <div className="max-w-3xl">
      <PageTitle title="Device" subtitle="The QDay connected to this browser." />
      <section className="rounded-3xl bg-surface px-6 pt-5 pb-2 sm:px-8 sm:pt-6">
        <div className="flex items-center gap-2.5 text-sm font-medium">
          <StatusDot tone="ok" /> Connected and ready to sign
        </div>
        <dl className="mt-4 divide-y divide-line/60 border-t border-line/60">
          {cap.left !== null && <Row label="Signatures left" value={groupThousands(cap.left)} />}
          <Row label="Device ID" value={shortHex(status.root, 8, 6)} mono />
        </dl>
        {cap.low && (
          <div className="mt-4 rounded-xl bg-warn/10 px-4 py-3 text-xs leading-relaxed text-warn">
            {cap.empty
              ? "This QDay has no signatures left on its current keys. Refresh its keys to keep signing."
              : "This QDay is running low on signatures. Refresh its keys soon."}
          </div>
        )}
        <div className="mt-5 flex items-center justify-between gap-3">
          <Button variant="secondary" onClick={onRefresh}>
            Refresh status
          </Button>
          <Button variant="ghost" className="-mr-3 px-3" onClick={onDisconnect}>
            Disconnect
          </Button>
        </div>
        <div className="mt-5 border-t border-line/60">
          <button
            onClick={() => setShowDetails((v) => !v)}
            aria-expanded={showDetails}
            className="flex w-full items-center justify-between py-3 text-sm text-muted transition-colors hover:text-alabaster"
          >
            Technical details
            <svg viewBox="0 0 16 16" className={`size-4 transition-transform ${showDetails ? "rotate-180" : ""}`} fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden>
              <path d="m4 6 4 4 4-4" />
            </svg>
          </button>
          {showDetails && (
            <div className="mb-1 rounded-2xl bg-onyx px-5 py-2">
              <dl className="divide-y divide-line/60">
                <Row label="Key set" value={String(status.tree)} />
                <Row label="Next signature" value={String(status.nextIndex)} />
                <Row label="Public key" value={status.root} mono />
                <Row label="Public seed" value={status.pubSeed} mono />
              </dl>
              <div className="mt-1 border-t border-line/60 pt-3 text-xs text-muted">Device log</div>
              <pre className="mt-2 mb-3 max-h-48 overflow-auto font-mono text-xs leading-relaxed text-muted">
                {log.length ? log.join("\n") : "Nothing logged yet."}
              </pre>
            </div>
          )}
        </div>
      </section>
    </div>
  );
}

function PageTitle({ title, subtitle }: { title: string; subtitle: string }) {
  return (
    <div className="mb-6">
      <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
      <p className="mt-1 text-sm text-muted">{subtitle}</p>
    </div>
  );
}

function Row({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="flex items-baseline justify-between gap-4 py-3 text-sm">
      <dt className="shrink-0 text-muted">{label}</dt>
      <dd className={`min-w-0 truncate text-right tabular-nums ${mono ? "font-mono text-xs" : ""}`} title={value}>
        {value}
      </dd>
    </div>
  );
}

function EmptyState({ title, body }: { title: string; body: string }) {
  return (
    <div className="py-10 text-center">
      <div className="text-sm font-semibold">{title}</div>
      <div className="mx-auto mt-1 max-w-xs text-sm text-muted">{body}</div>
    </div>
  );
}
