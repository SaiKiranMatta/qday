import { txUrl } from "../lib/explorer";
import type { Token } from "../lib/tokens";
import type { ActivityItem, ActivityKind } from "../lib/wallet";
import { TokenLogo } from "./TokenLogo";

const LABEL: Record<ActivityKind, string> = {
  sent: "Sent",
  received: "Received",
  created: "Account created",
  refresh: "Keys refreshed",
  refused: "Rejected on device",
};

/** Confirmed is the normal case, so only the exceptions get a word. */
const STATUS: Partial<Record<ActivityItem["status"], { text: string; tone: string }>> = {
  pending: { text: "Pending", tone: "text-warn" },
  failed: { text: "Failed", tone: "text-bad" },
};

const DAY_MS = 86_400_000;

/** Today, Yesterday, then dates; the year only when it isn't this one. */
function dayLabel(iso: string): string {
  const date = new Date(iso);
  const start = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const days = Math.round((start(new Date()) - start(date)) / DAY_MS);
  if (days === 0) return "Today";
  if (days === 1) return "Yesterday";
  const sameYear = date.getFullYear() === new Date().getFullYear();
  return date.toLocaleDateString("en-US", { month: "short", day: "numeric", ...(sameYear ? {} : { year: "numeric" }) });
}

function timeLabel(iso: string): string {
  return new Date(iso).toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" });
}

/** Transactions grouped by day, newest first. */
export function ActivityList({
  items,
  tokens,
  limit,
}: {
  items: ActivityItem[];
  tokens: Map<string, Token>;
  limit?: number;
}) {
  const shown = limit ? items.slice(0, limit) : items;
  const days = new Map<string, ActivityItem[]>();
  for (const item of shown) {
    const day = dayLabel(item.at);
    days.set(day, [...(days.get(day) ?? []), item]);
  }

  return (
    <div className="divide-y divide-line/60">
      {[...days].map(([day, rows]) => (
        <section key={day} className="py-2">
          <h3 className="pt-2 text-xs font-medium text-muted">{day}</h3>
          <ul className="divide-y divide-line/40">
            {rows.map((item) => (
              <li key={item.id}>
                {item.txHash ? (
                  <a
                    href={txUrl(item.txHash)}
                    target="_blank"
                    rel="noreferrer"
                    title="Open in explorer"
                    className="group -mx-3 flex items-center gap-4 rounded-2xl px-3 py-3.5 transition-colors hover:bg-surface-2/60 focus-visible:bg-surface-2/60"
                  >
                    <Row item={item} tokens={tokens} />
                    <svg
                      viewBox="0 0 16 16"
                      className="-ml-1 size-3.5 shrink-0 text-muted opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth="1.6"
                      aria-hidden
                    >
                      <path d="M5.5 10.5 10.5 5.5M6.5 5.5h4v4" />
                    </svg>
                  </a>
                ) : (
                  <div className="flex items-center gap-4 py-3.5">
                    <Row item={item} tokens={tokens} />
                  </div>
                )}
              </li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  );
}

/** One entry's icon, words and amount. The caller supplies the row, which links to the explorer when it can. */
function Row({ item, tokens }: { item: ActivityItem; tokens: Map<string, Token> }) {
  const token = item.amount ? tokens.get(item.amount.tokenId) : undefined;
  // A rejection never reached the chain, so it has no status of its own.
  const status = item.kind === "refused" ? undefined : STATUS[item.status];
  const struck = item.kind === "refused" || item.status === "failed";
  return (
    <>
      <ItemIcon item={item} token={token} />
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium">{LABEL[item.kind]}</div>
        <div className="truncate text-xs text-muted">
          {item.counterparty
            ? `${item.kind === "received" ? "From" : "To"} ${item.counterparty}`
            : item.kind === "created"
              ? "Your account is ready"
              : "New keys are active"}
        </div>
      </div>
      <div className="shrink-0 text-right">
        <div
          className={`text-sm font-medium tabular-nums ${item.kind === "received" ? "text-ok" : ""} ${
            struck ? "text-muted line-through decoration-muted/60" : ""
          }`}
        >
          {item.amount
            ? `${item.kind === "received" ? "+" : item.kind === "sent" ? "−" : ""}${item.amount.value} ${token?.symbol ?? ""}`
            : ""}
        </div>
        <div className="text-xs text-muted tabular-nums">
          {status && <span className={status.tone}>{status.text} · </span>}
          {timeLabel(item.at)}
        </div>
      </div>
    </>
  );
}

const PATH: Record<ActivityKind, string> = {
  sent: "M5 11 11 5M6 5h5v5",
  received: "M11 5 5 11M10 11H5V6",
  created: "M8 3v10M3 8h10",
  refresh: "M3.5 8a4.5 4.5 0 0 1 8-2.8M12.5 8a4.5 4.5 0 0 1-8 2.8M11.5 2.5v3h-3M4.5 13.5v-3h3",
  refused: "m5 5 6 6M11 5l-6 6",
};

/** The token's logo with a small badge for what happened; just the badge when no token moved. */
function ItemIcon({ item, token }: { item: ActivityItem; token: Token | undefined }) {
  const glyph = (size: string) => (
    <svg viewBox="0 0 16 16" className={size} fill="none" stroke="currentColor" strokeWidth="1.8">
      <path d={PATH[item.kind]} />
    </svg>
  );
  if (!item.amount) {
    return <span className="grid size-9 shrink-0 place-items-center rounded-full bg-surface-2 text-alabaster/80">{glyph("size-4")}</span>;
  }
  return (
    <span className="relative shrink-0">
      <TokenLogo token={token} size={36} />
      <span className="absolute -right-1 -bottom-1 grid size-[18px] place-items-center rounded-full bg-onyx text-alabaster ring-2 ring-surface">
        {glyph("size-2.5")}
      </span>
    </span>
  );
}

/** Placeholder rows while the first scan of the chain runs. */
export function ActivitySkeleton({ rows = 4 }: { rows?: number }) {
  return (
    <div className="py-2" role="status" aria-label="Loading activity">
      <div className="mt-2 h-3 w-16 animate-pulse rounded bg-surface-2" />
      <ul className="divide-y divide-line/40">
        {Array.from({ length: rows }, (_, i) => (
          <li key={i} className="flex items-center gap-4 py-3.5">
            <span className="size-9 shrink-0 animate-pulse rounded-full bg-surface-2" />
            <span className="flex-1 space-y-2">
              <span className="block h-3 w-20 animate-pulse rounded bg-surface-2" />
              <span className="block h-2.5 w-32 animate-pulse rounded bg-surface-2/70" />
            </span>
            <span className="space-y-2">
              <span className="ml-auto block h-3 w-16 animate-pulse rounded bg-surface-2" />
              <span className="ml-auto block h-2.5 w-12 animate-pulse rounded bg-surface-2/70" />
            </span>
          </li>
        ))}
      </ul>
    </div>
  );
}
