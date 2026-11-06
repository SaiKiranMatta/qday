import type { ReactNode } from "react";
import { Icon, type IconName } from "./icons";
import { Wordmark } from "./ui";

export type Page = "wallet" | "activity" | "device";

const NAV: { page: Page; label: string; icon: IconName }[] = [
  { page: "wallet", label: "Wallet", icon: "wallet" },
  { page: "activity", label: "Activity", icon: "activity" },
  { page: "device", label: "Device", icon: "device" },
];

/** Sidebar rows are h-10 with a 4px gap; the highlight slides by this much per row. */
const NAV_STEP_PX = 44;

/** Whether a QDay is attached, and what the sidebar can do about it. */
export type ShellDevice = { connected: true; onDisconnect: () => void } | { connected: false; connecting: boolean };

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
  const at = NAV.findIndex((item) => item.page === page);

  return (
    <div className="flex min-h-full">
      <aside className="sticky top-0 hidden h-screen w-60 shrink-0 flex-col border-r border-line/60 bg-[#0d0e10] px-4 py-6 md:flex">
        <Wordmark className="px-3 text-3xl" />
        <nav className="relative mt-10 flex flex-col gap-1" aria-label="Main">
          {/* One highlight that slides to the current page, so the move between pages reads as a move. */}
          {!locked && (
            <span
              aria-hidden
              className="absolute inset-x-0 top-0 h-10 rounded-xl bg-surface-2 transition-transform duration-300 ease-[var(--ease-out)] motion-reduce:transition-none"
              style={{ transform: `translateY(${at * NAV_STEP_PX}px)` }}
            />
          )}
          {NAV.map((item) => {
            const current = !locked && page === item.page;
            return (
              <button
                key={item.page}
                onClick={() => onNavigate(item.page)}
                disabled={locked}
                title={locked ? "Connect your QDay first" : undefined}
                aria-current={current ? "page" : undefined}
                className={`relative flex h-10 w-full items-center gap-3 rounded-xl px-3 text-sm transition-colors duration-200 disabled:cursor-not-allowed disabled:opacity-40 ${
                  current ? "font-medium text-alabaster" : "text-muted enabled:hover:text-alabaster"
                }`}
              >
                <Icon name={item.icon} className={`size-[18px] transition-opacity ${current ? "" : "opacity-80"}`} />
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
          <span
            aria-hidden
            className="absolute top-0 left-0 flex w-1/3 justify-center transition-transform duration-300 ease-[var(--ease-out)] motion-reduce:transition-none"
            style={{ transform: `translateX(${at * 100}%)` }}
          >
            <span className="h-0.5 w-8 rounded-full bg-alabaster" />
          </span>
          {NAV.map((item) => {
            const current = page === item.page;
            return (
              <button
                key={item.page}
                onClick={() => onNavigate(item.page)}
                aria-current={current ? "page" : undefined}
                className={`flex flex-1 flex-col items-center gap-1 pt-3 pb-2 text-[11px] font-medium transition-colors duration-200 ${
                  current ? "text-alabaster" : "text-muted"
                }`}
              >
                <Icon name={item.icon} className="size-5" />
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
      <span className={`relative ${device.connected ? "text-alabaster/80" : "text-muted"}`}>
        <Icon name="device" className="size-[18px]" />
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
      {device.connected && (
        <button
          onClick={device.onDisconnect}
          aria-label="Disconnect device"
          title="Disconnect"
          className="grid size-8 shrink-0 place-items-center rounded-lg text-muted transition-[color,background-color,transform] hover:bg-surface-2 hover:text-alabaster active:scale-95"
        >
          <Icon name="disconnect" />
        </button>
      )}
    </div>
  );
}
