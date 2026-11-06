import type { DeviceStatus } from "@qday/device";
import { useState } from "react";
import { ActivityList } from "./components/ActivityList";
import { ManageTokens } from "./components/ManageTokens";
import { TokenLogo } from "./components/TokenLogo";
import { Button, StatusDot } from "./components/ui";
import { capacity } from "./lib/capacity";
import { groupThousands, shortHex } from "./lib/format";
import type { TokenList } from "./lib/useTokens";
import type { AccountView } from "./lib/wallet";

/** Balance, assets and recent activity. */
export function WalletPage({
  account,
  status,
  tokens,
  onOpenActivity,
}: {
  account: AccountView;
  status: DeviceStatus;
  tokens: TokenList;
  onOpenActivity: () => void;
}) {
  const [managing, setManaging] = useState(false);
  return (
    <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_340px]">
      <div className="space-y-6">
        <section className="chrome-wide grain overflow-hidden rounded-3xl p-8">
          <div className="relative z-10 flex flex-wrap items-end justify-between gap-6">
            <div>
              <div className="text-sm text-alabaster/80">Total balance</div>
              <div className="mt-2 text-5xl font-semibold tracking-tight">
                {account.balance} <span className="text-alabaster/60">ETH</span>
              </div>
            </div>
            <div className="flex gap-3">
              <Button variant="onyx" disabled title="Coming next">
                Send
              </Button>
              <Button variant="onyx" disabled title="Coming next">
                Receive
              </Button>
            </div>
          </div>
        </section>

        <section className="rounded-3xl bg-surface p-6">
          <div className="flex items-center justify-between">
            <h2 className="text-base font-semibold">Assets</h2>
            <button onClick={() => setManaging(true)} className="text-xs text-muted hover:text-alabaster">
              Manage tokens
            </button>
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
      <aside className="space-y-6">
        <SecurityCard status={status} />
        <section className="rounded-3xl bg-surface p-6">
          <h2 className="text-base font-semibold">Receive address</h2>
          <p className="mt-3 break-all font-mono text-xs leading-relaxed text-alabaster/80">
            {account.address === "0x" ? "Not set up yet" : account.address}
          </p>
        </section>
      </aside>
    </div>
  );
}

/** The one security summary people need: protected, and how many signatures are left. */
function SecurityCard({ status }: { status: DeviceStatus }) {
  const cap = capacity(status);
  return (
    <section className="rounded-3xl bg-surface p-6">
      <h2 className="text-base font-semibold">Security</h2>
      <p className="mt-3 text-sm text-muted">
        Every transaction is approved on your QDay. No key on this computer can move your funds.
      </p>
      {cap.left !== null && (
        <div className="mt-5">
          <div className="flex items-baseline justify-between text-sm">
            <span className="text-muted">Signatures left</span>
            <span className="font-semibold">{groupThousands(cap.left)}</span>
          </div>
          {cap.low && (
            <div className="mt-3 rounded-xl bg-warn/10 p-3 text-xs text-warn">
              {cap.empty ? "Refresh your keys on the device to keep signing." : "Running low. Refresh your keys soon."}
            </div>
          )}
        </div>
      )}
    </section>
  );
}

export function ActivityPage({ account, tokens }: { account: AccountView; tokens: TokenList }) {
  return (
    <div className="max-w-3xl space-y-6">
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
    <div className="max-w-3xl space-y-6">
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
