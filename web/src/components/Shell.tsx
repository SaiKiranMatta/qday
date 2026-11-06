import type { ReactNode } from "react";
import { Wordmark } from "./ui";

export type Page = "wallet" | "activity" | "device";

const NAV: { page: Page; label: string; icon: ReactNode }[] = [
  { page: "wallet", label: "Wallet", icon: <IconWallet /> },
  { page: "activity", label: "Activity", icon: <IconActivity /> },
  { page: "device", label: "Device", icon: <IconDevice /> },
];

/** Whether a QDay is attached, and what the sidebar can do about it. */
export type ShellDevice =
  | { connected: true; onDisconnect: () => void }
  | { connected: false; connecting: boolean };

/** Sidebar and page area. Without a device the pages wait behind the connect panel. */
export function Shell({
  page,
  onNavigate,
  device,
  children,
}: {
  page: Page;
  onNavigate: (page: Page) => void;
  device: ShellDevice;
  children: ReactNode;
}) {
  const locked = !device.connected;
  return (
    <div className="flex min-h-full">
      <aside className="sticky top-0 hidden h-screen w-60 shrink-0 flex-col border-r border-line/60 bg-[#0d0e10] px-4 py-6 md:flex">
        <Wordmark className="px-3 text-3xl" />
        <nav className="mt-10 space-y-1" aria-label="Main">
          {NAV.map((item) => {
            const current = !locked && page === item.page;
            return (
              <button
                key={item.page}
                onClick={() => onNavigate(item.page)}
                disabled={locked}
                title={locked ? "Connect your QDay first" : undefined}
                aria-current={current ? "page" : undefined}
                className={`flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-sm transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${
                  current ? "bg-surface-2 font-medium text-alabaster" : "text-muted enabled:hover:text-alabaster"
                }`}
              >
                <span className={`size-4 ${current ? "" : "opacity-80"}`}>{item.icon}</span>
                {item.label}
              </button>
            );
          })}
        </nav>
        <DeviceRow device={device} />
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        <header className="px-5 pt-6 md:hidden">
          <Wordmark className="text-3xl" />
        </header>
        <main
          className={`mx-auto flex w-full max-w-6xl flex-1 flex-col px-5 pt-6 sm:px-8 md:pt-8 md:pb-12 ${locked ? "pb-10" : "pb-28"}`}
        >
          {children}
        </main>
      </div>

      {/* Phones: the sidebar is hidden, so the same pages sit in a tab bar once a device is connected. */}
      {!locked && (
        <nav
          aria-label="Main"
          className="fixed inset-x-0 bottom-0 z-40 flex border-t border-line/60 bg-[#0d0e10]/95 pb-[env(safe-area-inset-bottom)] backdrop-blur md:hidden"
        >
          {NAV.map((item) => {
            const current = page === item.page;
            return (
              <button
                key={item.page}
                onClick={() => onNavigate(item.page)}
                aria-current={current ? "page" : undefined}
                className={`flex flex-1 flex-col items-center gap-1 pt-2.5 pb-2 text-[11px] font-medium transition-colors ${
                  current ? "text-alabaster" : "text-muted"
                }`}
              >
                <span className="size-5">{item.icon}</span>
                {item.label}
              </button>
            );
          })}
        </nav>
      )}
    </div>
  );
}

/** The sidebar's footer: the connected device with a way out, or its absence (the page holds the way in). */
function DeviceRow({ device }: { device: ShellDevice }) {
  const dot = device.connected ? "bg-ok" : device.connecting ? "bg-warn animate-pulse" : "bg-muted";
  return (
    <div className="mt-auto flex items-center gap-3 rounded-xl bg-surface py-2.5 pr-2 pl-3">
      <span className={`relative size-4 ${device.connected ? "text-alabaster/80" : "text-muted"}`}>
        <IconDevice />
        <span className={`absolute -top-0.5 -right-0.5 size-2 rounded-full ring-2 ring-surface ${dot}`} />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block text-sm font-medium">
          {device.connected ? "QDay connected" : device.connecting ? "Connecting…" : "No device"}
        </span>
        <span className="block text-xs text-muted">
          {device.connected ? "Ready to sign" : device.connecting ? "Check your QDay" : "Not connected"}
        </span>
      </span>
      {device.connected ? (
        <button
          onClick={device.onDisconnect}
          aria-label="Disconnect device"
          title="Disconnect"
          className="grid size-8 shrink-0 place-items-center rounded-lg text-muted transition-colors hover:bg-surface-2 hover:text-alabaster"
        >
          <span className="size-4">
            <IconDisconnect />
          </span>
        </button>
      ) : null}
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
