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

const STATUS: Record<ActivityItem["status"], { text: string; tone: string }> = {
  confirmed: { text: "Confirmed", tone: "text-ok" },
  pending: { text: "Pending", tone: "text-warn" },
  failed: { text: "Not sent", tone: "text-muted" },
};

function dayLabel(iso: string): string {
  return new Date(iso).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" }).toUpperCase();
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
    <div className="space-y-5">
      {[...days].map(([day, rows]) => (
        <div key={day}>
          <div className="mb-2 text-[11px] font-medium tracking-wide text-muted">{day}</div>
          <div className="space-y-2">
            {rows.map((item) => (
              <div key={item.id} className="flex items-center gap-4 rounded-2xl bg-surface-2/60 px-4 py-3">
                <ItemIcon item={item} token={item.amount ? tokens.get(item.amount.tokenId) : undefined} />
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
                <div className="text-right">
                  <div className={`text-sm font-medium ${item.kind === "received" ? "text-ok" : ""}`}>
                    {item.amount
                      ? `${item.kind === "received" ? "+" : item.kind === "sent" ? "−" : ""}${item.amount.value} ${
                          tokens.get(item.amount.tokenId)?.symbol ?? ""
                        }`
                      : ""}
                  </div>
                  <div className="text-xs text-muted">
                    <span className={STATUS[item.status].tone}>{STATUS[item.status].text}</span> · {timeLabel(item.at)}
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
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
    return <span className="grid size-9 shrink-0 place-items-center rounded-full bg-surface text-alabaster/80">{glyph("size-4")}</span>;
  }
  return (
    <span className="relative shrink-0">
      <TokenLogo token={token} size={36} />
      <span className="absolute -right-1 -bottom-1 grid size-[18px] place-items-center rounded-full bg-onyx text-alabaster ring-2 ring-surface-2">
        {glyph("size-2.5")}
      </span>
    </span>
  );
}
