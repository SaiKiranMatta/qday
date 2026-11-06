import { txUrl } from "../lib/explorer";
import type { Token } from "../lib/tokens";
import type { ActivityItem, ActivityKind } from "../lib/wallet";
import { Icon, type IconName } from "./icons";
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

/** Wide screens lay the Activity page out as a table; these columns are shared by its header and rows. */
const TABLE = "xl:grid xl:grid-cols-[minmax(0,1.4fr)_minmax(0,1.2fr)_6rem_8rem_minmax(9rem,auto)_1rem] xl:items-center xl:gap-x-6";

/**
 * Transactions grouped by day, newest first. `table` spreads each row into columns on wide screens
 * (the Activity page); the default is the compact two-line row (cards, phones).
 */
export function ActivityList({
  items,
  tokens,
  limit,
  layout = "compact",
}: {
  items: ActivityItem[];
  tokens: Map<string, Token>;
  limit?: number;
  layout?: "compact" | "table";
}) {
  const shown = limit ? items.slice(0, limit) : items;
  // Rows ease in once, top first; the stagger is capped so a long list never makes anyone wait.
  const order = new Map(shown.map((item, i) => [item.id, i]));
  const days = new Map<string, ActivityItem[]>();
  for (const item of shown) {
    const day = dayLabel(item.at);
    days.set(day, [...(days.get(day) ?? []), item]);
  }
  const table = layout === "table";

  return (
    <div>
      {table && (
        <div className={`hidden border-b border-line/60 pt-1 pb-3 text-xs font-medium text-muted ${TABLE}`} aria-hidden>
          <span>Transaction</span>
          <span>From or to</span>
          <span>Time</span>
          <span>Status</span>
          <span className="text-right">Amount</span>
          <span />
        </div>
      )}
      <div className="divide-y divide-line/60">
        {[...days].map(([day, rows]) => (
          <section key={day} className="py-2">
            <h3 className="pt-2 text-xs font-medium text-muted">{day}</h3>
            <ul className="divide-y divide-line/40">
              {rows.map((item) => {
                const content = table ? (
                  <>
                    <span className="flex min-w-0 flex-1 items-center gap-4 xl:hidden">
                      <Row item={item} tokens={tokens} />
                    </span>
                    <TableRow item={item} tokens={tokens} linked={!!item.txHash} />
                  </>
                ) : (
                  <Row item={item} tokens={tokens} />
                );
                return (
                  <li
                    key={item.id}
                    className="animate-enter"
                    style={{ animationDelay: `${Math.min(order.get(item.id) ?? 0, 8) * 30}ms` }}
                  >
                    {item.txHash ? (
                      <a
                        href={txUrl(item.txHash)}
                        target="_blank"
                        rel="noreferrer"
                        className="group -mx-3 flex items-center gap-4 rounded-2xl px-3 py-3.5 transition-colors hover:bg-surface-2/60 focus-visible:bg-surface-2/60"
                      >
                        {content}
                      </a>
                    ) : (
                      <div className="flex items-center gap-4 py-3.5">{content}</div>
                    )}
                  </li>
                );
              })}
            </ul>
          </section>
        ))}
      </div>
    </div>
  );
}

/** What happened, in the compact form's words. */
function detail(item: ActivityItem): string {
  if (item.counterparty) return `${item.kind === "received" ? "From" : "To"} ${item.counterparty}`;
  return item.kind === "created" ? "Your account is ready" : "New keys are active";
}

function amountText(item: ActivityItem, token: Token | undefined): string {
  if (!item.amount) return "";
  const sign = item.kind === "received" ? "+" : item.kind === "sent" ? "−" : "";
  return `${sign}${item.amount.value} ${token?.symbol ?? ""}`;
}

function amountTone(item: ActivityItem): string {
  if (item.kind === "refused" || item.status === "failed") return "text-muted line-through decoration-muted/60";
  return item.kind === "received" ? "text-ok" : "";
}

/** One entry's icon, words and amount on two lines. The caller supplies the row, which links to the explorer when it can. */
function Row({ item, tokens }: { item: ActivityItem; tokens: Map<string, Token> }) {
  const token = item.amount ? tokens.get(item.amount.tokenId) : undefined;
  // A rejection never reached the chain, so it has no status of its own.
  const status = item.kind === "refused" ? undefined : STATUS[item.status];
  return (
    <>
      <ItemIcon item={item} token={token} />
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium">{LABEL[item.kind]}</div>
        <div className="truncate text-xs text-muted">{detail(item)}</div>
      </div>
      <div className="shrink-0 text-right">
        <div className={`text-sm font-medium tabular-nums ${amountTone(item)}`}>{amountText(item, token)}</div>
        <div className="text-xs text-muted tabular-nums">
          {status && <span className={status.tone}>{status.text} · </span>}
          {timeLabel(item.at)}
        </div>
      </div>
    </>
  );
}

const TABLE_STATUS: Record<ActivityItem["status"], { text: string; dot: string }> = {
  confirmed: { text: "Confirmed", dot: "bg-ok" },
  pending: { text: "Pending", dot: "bg-warn animate-pulse" },
  failed: { text: "Failed", dot: "bg-bad" },
};

/** The same entry spread over the table's columns, wide screens only. */
function TableRow({ item, tokens, linked }: { item: ActivityItem; tokens: Map<string, Token>; linked: boolean }) {
  const token = item.amount ? tokens.get(item.amount.tokenId) : undefined;
  const status = item.kind === "refused" ? { text: "Rejected", dot: "bg-muted" } : TABLE_STATUS[item.status];
  return (
    <span className={`hidden w-full ${TABLE}`}>
      <span className="flex min-w-0 items-center gap-4">
        <ItemIcon item={item} token={token} />
        <span className="min-w-0">
          <span className="block text-sm font-medium">{LABEL[item.kind]}</span>
          <span className="block truncate text-xs text-muted">{token?.name ?? (item.kind === "created" ? "Account" : "Keys")}</span>
        </span>
      </span>
      <span className="truncate text-sm text-muted">
        {item.counterparty ? `${item.kind === "received" ? "From" : "To"} ${item.counterparty}` : detail(item)}
      </span>
      <span className="text-sm text-muted tabular-nums">{timeLabel(item.at)}</span>
      <span className="flex items-center gap-2 text-sm text-muted">
        <span className={`size-1.5 rounded-full ${status.dot}`} />
        {status.text}
      </span>
      <span className={`text-right text-sm font-medium tabular-nums ${amountTone(item)}`}>{amountText(item, token)}</span>
      <span className="text-muted opacity-0 transition-[opacity,transform] duration-200 group-hover:translate-x-0.5 group-hover:-translate-y-0.5 group-hover:opacity-100 group-focus-visible:opacity-100">
        {linked && <Icon name="external" className="size-3.5" />}
      </span>
    </span>
  );
}

const GLYPH: Record<ActivityKind, IconName> = {
  sent: "send",
  received: "receive",
  created: "created",
  refresh: "refresh",
  refused: "refused",
};

/** The token's logo with a small badge for what happened; just the badge when no token moved. */
function ItemIcon({ item, token }: { item: ActivityItem; token: Token | undefined }) {
  if (!item.amount) {
    return (
      <span className="grid size-9 shrink-0 place-items-center rounded-full bg-surface-2 text-alabaster/80">
        <Icon name={GLYPH[item.kind]} className="size-4" />
      </span>
    );
  }
  return (
    <span className="relative shrink-0">
      <TokenLogo token={token} size={36} />
      <span className="absolute -right-1 -bottom-1 grid size-[18px] place-items-center rounded-full bg-onyx text-alabaster ring-2 ring-surface">
        <Icon name={GLYPH[item.kind]} className="size-2.5" />
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
