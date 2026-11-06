import type { DeviceStatus } from "@qday/device";
import type { ReactNode } from "react";
import { shortHex } from "../lib/format";
import type { AccountView } from "../lib/wallet";
import { StatusDot, Wordmark } from "./ui";

export type Page = "wallet" | "activity" | "device";

const NAV: { page: Page; label: string; icon: ReactNode }[] = [
  { page: "wallet", label: "Wallet", icon: <IconWallet /> },
  { page: "activity", label: "Activity", icon: <IconActivity /> },
  { page: "device", label: "Device", icon: <IconDevice /> },
];

/** Sidebar, top bar and page area, after the device is connected. */
export function Shell({
  page,
  onNavigate,
  status,
  account,
  onDisconnect,
  children,
}: {
  page: Page;
  onNavigate: (page: Page) => void;
  status: DeviceStatus;
  account: AccountView;
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
        <div className="mt-auto rounded-2xl bg-surface p-4">
          <div className="flex items-center justify-between">
            <span className="text-sm font-semibold">QDay device</span>
            <StatusDot tone="ok" />
          </div>
          <div className="mt-1 text-xs text-muted">Connected · ready to sign</div>
          <button onClick={onDisconnect} className="mt-3 text-xs text-muted hover:text-alabaster">
            Disconnect
          </button>
        </div>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="mx-auto flex w-full max-w-[52rem] items-center justify-between gap-4 px-6 py-5 sm:px-8">
          <AccountPill account={account} fallbackId={status.root} />
        </header>
        <main className="mx-auto w-full max-w-[52rem] flex-1 px-6 pb-10 sm:px-8">{children}</main>
      </div>
    </div>
  );
}

/** The current account. Becomes a switcher once there is more than one. */
function AccountPill({ account, fallbackId }: { account: AccountView; fallbackId: string }) {
  const id = account.address === "0x" ? fallbackId : account.address;
  return (
    <div className="flex items-center gap-3 rounded-2xl bg-surface py-2 pr-5 pl-2">
      <span className="chrome grain size-9 shrink-0 overflow-hidden rounded-full" />
      <span className="min-w-0">
        <span className="block text-sm font-semibold">{account.name}</span>
        <span className="block font-mono text-xs text-muted">{shortHex(id, 6, 4)}</span>
      </span>
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
