import type { DeviceStatus } from "@qday/device";
import { useState } from "react";
import { ActivityList } from "./components/ActivityList";
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
    <div>
      <div className="space-y-6">
        <section className="chrome-wide grain overflow-hidden rounded-3xl p-8">
          <div className="relative z-10 flex flex-wrap items-end justify-between gap-6">
            {setup ? (
              <SetupCard {...setup} />
            ) : (
              <div>
                <div className="flex items-center gap-2">
                  <span className="text-sm font-medium text-alabaster/90">{account.name}</span>
                  {account.address !== "0x" && <CopyAddress address={account.address} />}
                </div>
                <div className="mt-3 text-5xl font-semibold tracking-tight">
                  {chain === "loading" ? "…" : account.balance} <span className="text-alabaster/60">ETH</span>
                </div>
                {chain === "error" && <div className="mt-2 text-xs text-bad">Couldn't reach the devnet. Retrying.</div>}
              </div>
            )}
            <div className={`flex gap-3 ${setup ? "hidden" : ""}`}>
              <Button variant="onyx" disabled={!onSend} onClick={() => onSend?.()}>
                Send
              </Button>
              <Button variant="onyx" disabled={!onReceive} onClick={() => onReceive?.()}>
                Receive
              </Button>
            </div>
          </div>
        </section>

        {error && chain === "error" && (
          <div className="rounded-2xl bg-bad/10 p-4 text-sm text-bad">Couldn't read the devnet: {error}</div>
        )}

        <section className="rounded-3xl bg-surface p-6">
          <div className="flex items-center justify-between">
            <h2 className="text-base font-semibold">Assets</h2>
            <div className="flex items-center gap-4">
              {onMintTokens && (
                <button
                  onClick={() => void mint()}
                  disabled={minting === "busy"}
                  className={`text-xs hover:text-alabaster disabled:opacity-50 ${minting === "error" ? "text-bad" : "text-muted"}`}
                >
                  {minting === "busy" ? "Minting…" : minting === "error" ? "Couldn't mint. Retry" : "Get test tokens"}
                </button>
              )}
              <button onClick={() => setManaging(true)} className="text-xs text-muted hover:text-alabaster">
                Manage tokens
              </button>
            </div>
          </div>
          <div className="mt-4 divide-y divide-line/60">
            {tokens.selected.map((token) => (
              <div key={token.id} className="flex items-center gap-4 py-3">
                <TokenLogo token={token} />
                <div className="flex-1">
                  <div className="text-sm font-medium">{token.name}</div>
                  <div className="text-xs text-muted">{token.symbol}</div>
                </div>
                <div className="text-sm font-medium">
                  {account.balances[token.id] ?? "0"} <span className="text-muted">{token.symbol}</span>
                </div>
              </div>
            ))}
          </div>
        </section>

        <section className="rounded-3xl bg-surface p-6">
          <div className="flex items-center justify-between">
            <h2 className="text-base font-semibold">Recent activity</h2>
            {account.activity.length > 0 && (
              <button onClick={onOpenActivity} className="text-xs text-muted hover:text-alabaster">
                View all ›
              </button>
            )}
          </div>
          <div className="mt-4">
            {account.activity.length ? (
              <ActivityList items={account.activity} tokens={tokens.byId} limit={4} />
            ) : (
              <EmptyState title="Nothing yet" body="Transfers you approve on the device show up here." />
            )}
          </div>
        </section>
      </div>

      {managing && <ManageTokens tokens={tokens} onClose={() => setManaging(false)} />}
    </div>
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

export function ActivityPage({ account, tokens }: { account: AccountView; tokens: TokenList }) {
  return (
    <div className="max-w-3xl">
      <PageTitle title="Activity" subtitle="Everything this account sent and received" />
      <section className="rounded-3xl bg-surface p-6">
        {account.activity.length ? (
          <ActivityList items={account.activity} tokens={tokens.byId} />
        ) : (
          <EmptyState title="Nothing yet" body="Transfers you approve on the device show up here." />
        )}
      </section>
    </div>
  );
}

export function DevicePage({ status, log, onRefresh }: { status: DeviceStatus; log: string[]; onRefresh: () => void }) {
  const [showDetails, setShowDetails] = useState(false);
  const cap = capacity(status);
  return (
    <div className="max-w-3xl">
      <PageTitle title="Device" subtitle="Your connected QDay" />
      <section className="rounded-3xl bg-surface p-6">
        <div className="flex items-center gap-2 text-sm">
          <StatusDot tone="ok" /> Connected · ready to sign
        </div>
        {cap.left !== null && <Row label="Signatures left" value={groupThousands(cap.left)} />}
        <Row label="Device ID" value={shortHex(status.root, 8, 6)} mono />
        <div className="mt-5 flex gap-3">
          <Button variant="secondary" onClick={onRefresh}>
            Refresh
          </Button>
          <Button variant="ghost" onClick={() => setShowDetails((v) => !v)}>
            {showDetails ? "Hide" : "Show"} technical details
          </Button>
        </div>
        {showDetails && (
          <div className="mt-4 space-y-3 rounded-xl bg-onyx p-4">
            <Row label="Key set" value={String(status.tree)} />
            <Row label="Next signature" value={String(status.nextIndex)} />
            <Row label="Public key" value={status.root} mono />
            <Row label="Public seed" value={status.pubSeed} mono />
            <div className="text-xs text-muted">Device log</div>
            <pre className="max-h-48 overflow-auto font-mono text-xs text-muted">
              {log.length ? log.join("\n") : "Nothing logged yet."}
            </pre>
          </div>
        )}
      </section>
    </div>
  );
}

function PageTitle({ title, subtitle }: { title: string; subtitle: string }) {
  return (
    <div>
      <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
      <p className="mt-1 text-sm text-muted">{subtitle}</p>
    </div>
  );
}

function Row({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="mt-3 flex items-baseline justify-between gap-4 border-b border-line/50 pb-3 text-sm last:border-0">
      <span className="shrink-0 text-muted">{label}</span>
      <span className={`min-w-0 truncate text-right ${mono ? "font-mono text-xs" : ""}`} title={value}>
        {value}
      </span>
    </div>
  );
}

function EmptyState({ title, body }: { title: string; body: string }) {
  return (
    <div className="py-10 text-center">
      <div className="text-sm font-semibold">{title}</div>
      <div className="mt-1 text-sm text-muted">{body}</div>
    </div>
  );
}
