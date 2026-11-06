import { useState } from "react";
import { hasDemoWallet } from "../lib/demo";
import type { ConnectionState } from "../lib/useDevice";
import { DemoIntro } from "./DemoIntro";
import { DeviceArt } from "./DeviceArt";
import { Icon } from "./icons";
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
  onTryDemo,
}: {
  state: Exclude<ConnectionState, { kind: "connected" }>;
  onConnect: () => void;
  onTryDemo: (seed?: `0x${string}`) => void;
}) {
  const [intro, setIntro] = useState(false);
  const unsupported = state.kind === "unsupported";
  const connecting = state.kind === "connecting";

  return (
    <div className="grid flex-1 place-items-center py-6 md:py-10">
      <section className="w-full max-w-md animate-pop rounded-3xl bg-surface p-7 sm:p-9" aria-labelledby="connect-title">
        <DeviceArt light={connecting ? "busy" : "idle"} />
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

        <Button
          variant={unsupported ? "primary" : "secondary"}
          className="mt-3 w-full"
          disabled={connecting}
          onClick={() => setIntro(true)}
        >
          Try without a device
        </Button>

        <p className="mt-7 flex items-start gap-2.5 border-t border-line/60 pt-5 text-xs leading-relaxed text-muted">
          <Icon name="lock" className="mt-px size-4 text-alabaster/70" />
          On a QDay, your keys never leave the device, and every transfer is approved on it.
        </p>
      </section>
      {intro && (
        <DemoIntro
          saved={hasDemoWallet()}
          onStart={(seed) => {
            setIntro(false);
            onTryDemo(seed);
          }}
          onClose={() => setIntro(false)}
        />
      )}
    </div>
  );
}
