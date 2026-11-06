import type { ConnectionState } from "../lib/useDevice";
import { Button, StatusDot } from "./ui";

const STEPS = [
  <>
    Plug your QDay into this computer using the port labelled <span className="text-alabaster">USB</span>.
  </>,
  <>Unlock it with your passcode.</>,
  <>
    Select <span className="text-alabaster">Connect device</span>, then choose your QDay in the list your browser shows.
  </>,
];

/** Stands in for the wallet until a QDay is attached: what to do, and the one button that does it. */
export function ConnectPanel({
  state,
  onConnect,
}: {
  state: Exclude<ConnectionState, { kind: "connected" }>;
  onConnect: () => void;
}) {
  const unsupported = state.kind === "unsupported";
  const connecting = state.kind === "connecting";

  return (
    <div className="grid flex-1 place-items-center py-6 md:py-10">
      <section className="w-full max-w-md rounded-3xl bg-surface p-7 sm:p-9" aria-labelledby="connect-title">
        <DeviceMark busy={connecting} />
        <h1 id="connect-title" className="mt-7 text-2xl font-semibold tracking-tight">
          {unsupported ? "This browser can't connect to QDay" : "Connect your QDay"}
        </h1>
        <p className="mt-2 text-sm leading-relaxed text-muted">
          {unsupported
            ? "QDay connects over USB, which needs Chrome, Edge or Brave on a desktop computer. Open this page in one of them to continue."
            : "Your QDay is your wallet. Connect it to see your balance, send and receive."}
        </p>

        {!unsupported && (
          <>
            <ol className="mt-6 space-y-3 border-t border-line/60 pt-6">
              {STEPS.map((step, i) => (
                <li key={i} className="flex gap-3 text-sm leading-relaxed text-muted">
                  <span className="grid size-6 shrink-0 place-items-center rounded-full bg-surface-2 text-xs font-semibold text-alabaster tabular-nums">
                    {i + 1}
                  </span>
                  <span className="pt-0.5">{step}</span>
                </li>
              ))}
            </ol>

            <Button className="mt-7 w-full" onClick={onConnect} disabled={connecting}>
              {connecting ? "Connecting…" : "Connect device"}
            </Button>
            <p className="mt-3 text-center text-xs text-muted" aria-live="polite">
              {connecting ? "Waiting for your QDay to respond." : "Works in Chrome, Edge and Brave on desktop."}
            </p>

            {state.kind === "error" && (
              <div role="alert" className="mt-5 flex items-start gap-2.5 rounded-xl bg-bad/10 px-4 py-3 text-sm text-bad">
                <span className="mt-1.5">
                  <StatusDot tone="bad" />
                </span>
                <span>{state.message}</span>
              </div>
            )}
          </>
        )}

        <p className="mt-7 flex items-start gap-2.5 border-t border-line/60 pt-5 text-xs leading-relaxed text-muted">
          <svg viewBox="0 0 16 16" className="mt-px size-4 shrink-0 text-alabaster/70" fill="none" stroke="currentColor" strokeWidth="1.4" aria-hidden>
            <rect x="3" y="7" width="10" height="7" rx="1.6" />
            <path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2" />
          </svg>
          Your keys never leave the device, and every transfer is approved on it.
        </p>
      </section>
    </div>
  );
}

/** A small QDay in the brand's chrome: the device the steps talk about. */
function DeviceMark({ busy }: { busy: boolean }) {
  return (
    <span className="chrome grain relative block h-[72px] w-14 overflow-hidden rounded-2xl" aria-hidden>
      <span className="absolute inset-x-2 top-2 z-10 h-9 rounded-lg bg-onyx/85">
        <span className={`absolute top-1.5 left-1.5 size-1.5 rounded-full ${busy ? "animate-pulse bg-warn" : "bg-alabaster/40"}`} />
      </span>
      <span className="absolute inset-x-3.5 bottom-3 z-10 flex justify-between">
        <span className="size-2 rounded-full bg-onyx/60" />
        <span className="size-2 rounded-full bg-onyx/60" />
      </span>
    </span>
  );
}
