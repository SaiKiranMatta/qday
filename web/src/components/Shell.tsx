import type { ReactNode } from "react";
import { Wordmark } from "./ui";

export type Page = "wallet" | "activity" | "device";

const NAV: { page: Page; label: string; icon: ReactNode }[] = [
  { page: "wallet", label: "Wallet", icon: <IconWallet /> },
  { page: "activity", label: "Activity", icon: <IconActivity /> },
  { page: "device", label: "Device", icon: <IconDevice /> },
];

/** Sidebar and page area, after the device is connected. */
export function Shell({
  page,
  onNavigate,
  onDisconnect,
  children,
}: {
  page: Page;
  onNavigate: (page: Page) => void;
  onDisconnect: () => void;
  children: ReactNode;
}) {
  return (
    <div className="flex min-h-full">
      <aside className="sticky top-0 hidden h-screen w-60 shrink-0 flex-col border-r border-line/60 bg-[#0d0e10] px-4 py-6 md:flex">
        <Wordmark className="px-3 text-3xl" />
        <nav className="mt-10 space-y-1">
          {NAV.map((item) => (
            <button
              key={item.page}
              onClick={() => onNavigate(item.page)}
              className={`flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-sm transition-colors ${
                page === item.page ? "bg-surface-2 text-alabaster" : "text-muted hover:bg-surface hover:text-alabaster"
              }`}
            >
              <span className="size-4">{item.icon}</span>
              {item.label}
            </button>
          ))}
        </nav>
        <div className="mt-auto flex items-center gap-3 rounded-xl bg-surface py-2.5 pr-2 pl-3">
          <span className="relative size-4 text-alabaster/80">
            <IconDevice />
            <span className="absolute -top-0.5 -right-0.5 size-2 rounded-full bg-ok ring-2 ring-surface" />
          </span>
          <span className="min-w-0 flex-1">
            <span className="block text-sm font-medium">QDay connected</span>
            <span className="block text-xs text-muted">Ready to sign</span>
          </span>
          <button
            onClick={onDisconnect}
            aria-label="Disconnect device"
            title="Disconnect"
            className="grid size-8 shrink-0 place-items-center rounded-lg text-muted transition-colors hover:bg-surface-2 hover:text-alabaster"
          >
            <span className="size-4">
              <IconDisconnect />
            </span>
          </button>
        </div>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="px-6 pt-6 md:hidden">
          <Wordmark className="text-3xl" />
        </header>
        <main className="mx-auto w-full max-w-[52rem] flex-1 px-6 pt-6 pb-10 sm:px-8">{children}</main>
      </div>
    </div>
  );
}

function IconWallet() {
  return (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" className="size-full">
      <rect x="1.5" y="3.5" width="13" height="10" rx="2" />
      <path d="M10.5 8.5h2M1.5 6h13" />
    </svg>
  );
}
function IconActivity() {
  return (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" className="size-full">
      <path d="M1.5 8h3l2-5 3 10 2-5h3" />
    </svg>
  );
}
function IconDevice() {
  return (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" className="size-full">
      <rect x="3.5" y="1.5" width="9" height="13" rx="2" />
      <path d="M6.5 11.5h3" />
    </svg>
  );
}
function IconDisconnect() {
  return (
    <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" className="size-full">
      <path d="M6.5 2.5h-3a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h3M10.5 5l3 3-3 3M13.5 8h-7" />
    </svg>
  );
}
