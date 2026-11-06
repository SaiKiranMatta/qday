import { useState } from "react";
import { type Page, Shell } from "./components/Shell";
import { Badge, Button, StatusDot, Wordmark } from "./components/ui";
import { useDevice } from "./lib/useDevice";
import { useTokens } from "./lib/useTokens";
import { EMPTY_ACCOUNT, MOCK, MOCK_ACCOUNT } from "./lib/wallet";
import { ActivityPage, DevicePage, WalletPage } from "./pages";

export function App() {
  const { state, log, connect, refresh, disconnect } = useDevice();
  const [page, setPage] = useState<Page>("wallet");
  const account = MOCK ? MOCK_ACCOUNT : EMPTY_ACCOUNT;
  const tokens = useTokens();

  if (state.kind === "connected") {
    const { status } = state;
    return (
      <Shell page={page} onNavigate={setPage} status={status} account={account} onDisconnect={disconnect}>
        {page === "wallet" && <WalletPage account={account} status={status} tokens={tokens} onOpenActivity={() => setPage("activity")} />}
        {page === "activity" && <ActivityPage account={account} tokens={tokens} />}
        {page === "device" && <DevicePage status={status} log={log} onRefresh={refresh} />}
      </Shell>
    );
  }

  return (
    <div className="flex min-h-full flex-col">
      <header className="flex items-center justify-between px-6 py-5 sm:px-10">
        <Wordmark className="text-3xl" />
        <Badge>
          <StatusDot tone="idle" /> Devnet · chain 1337
        </Badge>
      </header>

      <main className="grid flex-1 gap-6 px-6 pb-10 sm:px-10 lg:grid-cols-[minmax(0,5fr)_minmax(0,6fr)]">
        <section className="chrome grain flex min-h-[420px] flex-col justify-between overflow-hidden rounded-3xl p-8 sm:p-10">
          <p className="relative z-10 ml-auto max-w-sm text-right text-sm leading-relaxed text-alabaster/90">
            <span className="font-semibold italic">QDay</span> is a quantum-safe hardware wallet for Ethereum. Your keys
            are quantum-safe and never leave the device.
          </p>
          <Wordmark className="relative z-10 text-7xl text-slate sm:text-8xl" />
        </section>

        <section className="flex flex-col justify-center">
          <ConnectCard state={state} onConnect={connect} />
        </section>
      </main>
    </div>
  );
}

function ConnectCard({
  state,
  onConnect,
}: {
  state: Exclude<ReturnType<typeof useDevice>["state"], { kind: "connected" }>;
  onConnect: () => void;
}) {
  if (state.kind === "unsupported") {
    return (
      <div className="rounded-3xl bg-surface p-8">
        <h1 className="text-2xl font-semibold tracking-tight">This browser can't reach your QDay</h1>
        <p className="mt-3 text-sm text-muted">
          QDay talks to the device over USB with Web Serial. Open this page in Chrome, Edge or Brave on a computer.
        </p>
      </div>
    );
  }

  return (
    <div className="rounded-3xl bg-surface p-8">
      <h1 className="text-3xl font-semibold tracking-tight">Connect your QDay</h1>
      <p className="mt-3 max-w-md text-sm leading-relaxed text-muted">
        Plug the device into the port labelled <span className="text-alabaster">USB</span>, unlock it, then pick it from
        the list your browser shows.
      </p>
      <div className="mt-8 flex items-center gap-4">
        <Button onClick={onConnect} disabled={state.kind === "connecting"}>
          {state.kind === "connecting" ? "Connecting…" : "Connect device"}
        </Button>
        {state.kind === "connecting" && <span className="text-xs text-muted">Waiting for the device to answer</span>}
      </div>
      {state.kind === "error" && (
        <div className="mt-6 flex items-start gap-2 rounded-xl bg-bad/10 p-4 text-sm text-bad">
          <StatusDot tone="bad" />
          <span>{state.message}</span>
        </div>
      )}
    </div>
  );
}
