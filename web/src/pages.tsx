import type { DeviceStatus, SchemeId } from "@qday/device";
import { type ComponentProps, type ReactNode, useState } from "react";
import { keccak256 } from "viem";
import { ActivityList, ActivitySkeleton } from "./components/ActivityList";
import { ManageTokens } from "./components/ManageTokens";
import { TokenLogo } from "./components/TokenLogo";
import { Button } from "./components/ui";
import { capacity } from "./lib/capacity";
import { SCHEMES } from "./lib/schemes";
import type { PrivateNote, SetupState } from "./lib/useStealth";
import { exactEth } from "./lib/format";
import { groupThousands, shortHex } from "./lib/format";
import { DeviceArt } from "./components/DeviceArt";
import { Icon } from "./components/icons";
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
  privatePayments,
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
    scheme: SchemeId;
    demo: boolean;
    onFund: () => void;
    onCreate: () => void;
  } | null;
  /** Null while sending isn't possible (no account yet, or no device in mock mode). */
  onSend: (() => void) | null;
  onReceive: (() => void) | null;
  /** Local devnet only: mints test tokens to the account. */
  onMintTokens: (() => Promise<void>) | null;
  onOpenActivity: () => void;
  /** Null until there's an account to claim to. */
  privatePayments: {
    setup: SetupState;
    notes: PrivateNote[];
    scanned: boolean;
    demo: boolean;
    onEnable: () => void;
    onClaim: (note: PrivateNote) => void;
  } | null;
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
              {/* Keyed by the amount, so a new balance ticks in and the change is noticed. */}
              <div key={account.balance} className="mt-3 animate-tick text-5xl font-semibold tracking-tight tabular-nums">
                {chain === "loading" ? "…" : account.balance} <span className="text-alabaster/60">ETH</span>
              </div>
            </div>
          )}
          {!setup && (
            <div className="flex gap-3">
              <Button variant="onyx" className="pr-6 pl-5" disabled={!onSend} onClick={() => onSend?.()}>
                <Icon name="send" className="size-4" />
                Send
              </Button>
              <Button variant="onyx" className="pr-6 pl-5" disabled={!onReceive} onClick={() => onReceive?.()}>
                <Icon name="receive" className="size-4" />
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

      {privatePayments && <PrivatePayments {...privatePayments} />}

      {managing && <ManageTokens tokens={tokens} onClose={() => setManaging(false)} />}
    </div>
  );
}

/** Payments to the stealth address, found by scanning the pool. Claiming sends a whole note out. */
function PrivatePayments({
  setup,
  notes,
  scanned,
  demo,
  onEnable,
  onClaim,
}: {
  setup: SetupState;
  notes: PrivateNote[];
  scanned: boolean;
  demo: boolean;
  onEnable: () => void;
  onClaim: (note: PrivateNote) => void;
}) {
  const waiting = notes.filter((note) => !note.spent);
  return (
    <Card title="Private payments">
      {setup.kind !== "on" ? (
        <div className="py-3">
          <p className="max-w-xl text-sm leading-relaxed text-muted">
            A private address lets anyone pay you without the payment pointing to this account.{" "}
            {demo
              ? "In this demo wallet, setting it up takes one click."
              : "Your QDay asks you to share the address, then the key that finds payments, which can't spend them. This page forgets that key when you leave."}
          </p>
          {setup.kind === "error" && (
            <p role="alert" className="mt-3 text-xs text-bad">
              {setup.message}
            </p>
          )}
          <Button variant="secondary" className="mt-4" disabled={setup.kind === "sharing"} onClick={onEnable}>
            {setup.kind === "sharing" ? "Setting up…" : demo ? "Set up" : "Set up on your QDay"}
          </Button>
        </div>
      ) : !scanned ? (
        <p className="py-3 text-sm text-muted">Looking for payments…</p>
      ) : waiting.length === 0 ? (
        <EmptyState title="No private payments waiting" body="Open Receive to copy your private address." />
      ) : (
        <ul className="divide-y divide-line/60">
          {waiting.map((note) => (
            <li key={note.keyHash} className="flex items-center gap-4 py-3.5">
              <div className="min-w-0 flex-1">
                <div className="text-sm font-medium tabular-nums">
                  {exactEth(note.valueWei)} <span className="text-muted">ETH</span>
                </div>
                <div className="truncate font-mono text-xs text-muted">{shortHex(note.keyHash, 8, 6)}</div>
              </div>
              <Button variant="secondary" className="h-9" onClick={() => onClaim(note)}>
                Claim
              </Button>
            </li>
          ))}
        </ul>
      )}
    </Card>
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
      className="flex items-center gap-1.5 rounded-full bg-onyx/40 py-1 pr-2 pl-2.5 font-mono text-xs text-alabaster/70 transition-[color,background-color,transform] hover:bg-onyx/60 hover:text-alabaster active:scale-95"
    >
      {copied ? <span className="font-sans">Copied</span> : shortHex(address, 6, 4)}
      <Icon name={copied ? "check" : "copy"} className={`size-3 ${copied ? "text-ok" : ""}`} />
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
    <div>
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
            <ActivityList items={items} tokens={tokens.byId} limit={shown} layout="table" />
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
  demo,
  onRefresh,
  onDisconnect,
}: {
  status: DeviceStatus;
  log: string[];
  /** Present for the demo wallet, which has no device behind it. */
  demo: { onExportSeed: () => string | null; onForget: () => void } | null;
  onRefresh: () => void;
  onDisconnect: () => void;
}) {
  const [showDetails, setShowDetails] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const cap = capacity(status);
  const ml = status.scheme === "ml-dsa-44";
  // The ID the device's own Wallet tab shows.
  const deviceId = ml ? keccak256(status.publicKey) : status.root;
  const refresh = async () => {
    setRefreshing(true);
    try {
      await onRefresh();
    } finally {
      setTimeout(() => setRefreshing(false), 400);
    }
  };

  return (
    <div>
      <PageTitle title="Device" subtitle={demo ? "The demo wallet in this browser." : "The QDay connected to this browser."} />

      <section className="flex flex-col gap-6 rounded-3xl bg-surface px-6 py-6 sm:flex-row sm:items-center sm:gap-8 sm:px-8 sm:py-7">
        <DeviceArt size="large" light="ok" caption={demo ? "Demo" : "Ready to sign"} />
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2.5 text-sm font-medium text-ok">
            <span className="size-2 animate-breathe rounded-full bg-ok" /> {demo ? "Ready to sign in this browser" : "Connected and ready to sign"}
          </div>
          <h2 className="mt-2 text-2xl font-semibold tracking-tight">{demo ? "Demo wallet" : "QDay"}</h2>
          <div className="mt-1 flex items-center gap-2 text-sm text-muted">
            Device ID <span className="font-mono text-xs text-alabaster/80">{shortHex(deviceId, 8, 6)}</span>
            <CopyIconButton text={deviceId} label="Copy device ID" />
          </div>
          <div className="mt-5 flex flex-wrap items-center gap-3">
            <Button variant="secondary" onClick={() => void refresh()} disabled={refreshing}>
              <Icon name="refresh" className={`size-4 ${refreshing ? "animate-spin" : ""}`} />
              {refreshing ? "Refreshing…" : "Refresh status"}
            </Button>
            <Button variant="ghost" className="px-4" onClick={onDisconnect}>
              <Icon name="disconnect" className="size-4" />
              Disconnect
            </Button>
          </div>
        </div>
      </section>

      <div className="mt-6 grid gap-6 lg:grid-cols-2 lg:items-start">
        {cap.left !== null && cap.total !== null && (
          <section className="rounded-3xl bg-surface px-6 py-5 sm:px-8 sm:py-6">
            <h2 className="text-base font-semibold">Signatures</h2>
            <div className="mt-4 flex items-baseline gap-2">
              <span className={`text-4xl font-semibold tracking-tight tabular-nums ${cap.empty ? "text-bad" : cap.low ? "text-warn" : ""}`}>
                {groupThousands(cap.left)}
              </span>
              <span className="text-sm text-muted">left of {groupThousands(cap.total)}</span>
            </div>
            <div
              className="mt-4 h-2 overflow-hidden rounded-full bg-surface-2"
              role="meter"
              aria-label="Signatures left"
              aria-valuemin={0}
              aria-valuemax={cap.total}
              aria-valuenow={cap.left}
            >
              <div
                className={`h-full origin-left animate-grow rounded-full ${cap.empty ? "bg-bad" : cap.low ? "bg-warn" : "bg-alabaster"}`}
                style={{ width: `${Math.max((cap.left / cap.total) * 100, cap.left > 0 ? 1.5 : 0)}%` }}
              />
            </div>
            <p className="mt-4 text-xs leading-relaxed text-muted">
              {demo
                ? "Each approval uses one signature. The demo wallet can't refresh its keys, so start a new one when they run out."
                : "Each approval uses one signature. When they run low, refresh the keys on your QDay. Your address stays the same."}
            </p>
            {cap.low && !demo && (
              <div className="mt-4 rounded-xl bg-warn/10 px-4 py-3 text-xs leading-relaxed text-warn">
                {cap.empty
                  ? "No signatures left on these keys. Refresh the keys on your QDay to keep signing."
                  : "Running low. Refresh the keys on your QDay soon."}
              </div>
            )}
          </section>
        )}

        <section className="rounded-3xl bg-surface px-6 pt-5 pb-2 sm:px-8 sm:pt-6">
          <h2 className="text-base font-semibold">Details</h2>
          <dl className="mt-2 divide-y divide-line/60">
            <Row label="Connection" value={demo ? "This browser" : "USB"} />
            <Row label="Signing" value={SCHEMES[status.scheme].name} />
            {!ml && <Row label="Key set" value={`#${status.tree + 1}`} />}
            {!ml && <Row label="Next signature" value={`#${String(status.nextIndex).padStart(4, "0")}`} />}
          </dl>
          <div className="border-t border-line/60">
            <button
              onClick={() => setShowDetails((v) => !v)}
              aria-expanded={showDetails}
              className="flex w-full items-center justify-between py-3 text-sm text-muted transition-colors hover:text-alabaster"
            >
              Technical details
              <Icon name="chevron" className={`size-4 transition-transform duration-200 ${showDetails ? "rotate-180" : ""}`} />
            </button>
            {showDetails && (
              <div className="mb-4 animate-enter rounded-2xl bg-onyx px-4 py-2">
                <dl className="divide-y divide-line/60">
                  <Row label="Public key" value={ml ? status.publicKey : status.root} mono />
                  {!ml && <Row label="Public seed" value={status.pubSeed} mono />}
                </dl>
                <div className="border-t border-line/60 pt-3 text-xs text-muted">Device log</div>
                <pre className="mt-2 mb-3 max-h-48 overflow-auto font-mono text-xs leading-relaxed text-muted">
                  {log.length ? log.join("\n") : "Nothing logged yet."}
                </pre>
              </div>
            )}
          </div>
        </section>
      </div>

      {demo && <DemoControls {...demo} />}
    </div>
  );
}

/** The demo wallet's seed, shown only on request, and a way to delete the wallet. */
function DemoControls({ onExportSeed, onForget }: { onExportSeed: () => string | null; onForget: () => void }) {
  const [seed, setSeed] = useState<string | null>(null);
  const [forgetting, setForgetting] = useState(false);
  return (
    <section className="mt-6 rounded-3xl bg-surface px-6 py-5 sm:px-8 sm:py-6">
      <h2 className="text-base font-semibold">Demo wallet</h2>
      <p className="mt-2 max-w-xl text-sm leading-relaxed text-muted">
        Clearing your browser data deletes this wallet. Its seed is the only way to get it back, and anyone with the seed
        has the wallet.
      </p>
      {seed && (
        <div className="mt-4 flex items-center gap-2 rounded-2xl bg-surface-2 px-4 py-3">
          <code className="min-w-0 flex-1 font-mono text-xs break-all">{seed}</code>
          <CopyIconButton text={seed} label="Copy seed" />
        </div>
      )}
      <div className="mt-4 flex flex-wrap gap-3">
        <Button variant="secondary" onClick={() => setSeed(seed ? null : onExportSeed())}>
          {seed ? "Hide seed" : "Show seed"}
        </Button>
        {forgetting ? (
          <>
            <Button variant="secondary" className="text-bad" onClick={onForget}>
              Delete it for good
            </Button>
            <Button variant="ghost" onClick={() => setForgetting(false)}>
              Keep it
            </Button>
          </>
        ) : (
          <Button variant="ghost" onClick={() => setForgetting(true)}>
            Delete this wallet
          </Button>
        )}
      </div>
    </section>
  );
}

/** A small copy control with an icon that confirms the copy. */
function CopyIconButton({ text, label }: { text: string; label: string }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard blocked; the technical details show the full value.
    }
  };
  return (
    <button
      onClick={() => void copy()}
      aria-label={copied ? "Copied" : label}
      title={copied ? "Copied" : label}
      className={`grid size-7 place-items-center rounded-lg transition-[color,background-color,transform] hover:bg-surface-2 active:scale-90 ${
        copied ? "text-ok" : "text-muted hover:text-alabaster"
      }`}
    >
      <Icon name={copied ? "check" : "copy"} className="size-3.5" />
    </button>
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
    <div className="flex flex-col items-center py-10 text-center">
      <span className="grid size-11 place-items-center rounded-full bg-surface-2 text-muted">
        <Icon name="activity" className="size-5" />
      </span>
      <div className="mt-4 text-sm font-semibold">{title}</div>
      <div className="mx-auto mt-1 max-w-xs text-sm text-muted">{body}</div>
    </div>
  );
}
