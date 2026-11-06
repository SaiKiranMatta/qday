import { useState } from "react";
import { Modal } from "./Modal";
import { Button } from "./ui";

const SEED = /^0x[0-9a-fA-F]{64}$/;

/** Says what the demo wallet is before there is one, and takes a saved seed. */
export function DemoIntro({
  saved,
  onStart,
  onClose,
}: {
  /** True when this browser already holds a demo wallet. */
  saved: boolean;
  onStart: (seed?: `0x${string}`) => void;
  onClose: () => void;
}) {
  const [restoring, setRestoring] = useState(false);
  const [seed, setSeed] = useState("");
  const valid = SEED.test(seed);

  return (
    <Modal title="Try QDay without a device" onClose={onClose}>
      <p className="text-sm leading-relaxed text-alabaster/90">
        This wallet runs in your browser with test ETH, so you can create an account, send and try private payments.
      </p>
      <ul className="mt-4 space-y-2 rounded-2xl bg-surface-2 px-4 py-3.5 text-xs leading-relaxed text-muted">
        <li>Your keys live in this browser, not on a device, so anything that reaches this page can reach them.</li>
        <li>Clearing your browser data deletes the wallet. Save the seed from the Device page if you want to keep it.</li>
        <li>Never use it with real funds. It only works on the test network.</li>
      </ul>

      {restoring && (
        <div className="mt-4">
          <label htmlFor="demo-seed" className="text-xs text-muted">
            Demo wallet seed
          </label>
          <input
            id="demo-seed"
            value={seed}
            onChange={(e) => setSeed(e.target.value.trim())}
            autoComplete="off"
            spellCheck={false}
            placeholder="0x…"
            className="mt-2 w-full rounded-2xl bg-surface-2 px-4 py-3 font-mono text-xs outline-none placeholder:text-muted focus:ring-1 focus:ring-alabaster/25"
          />
          <p className="mt-2 text-xs text-muted">
            {seed && !valid
              ? "A seed is 0x and 64 hex digits."
              : saved
                ? "This replaces the demo wallet in this browser."
                : "Don't open the same seed in two browsers at once."}
          </p>
        </div>
      )}

      <Button
        className="mt-5 w-full"
        disabled={restoring && !valid}
        onClick={() => onStart(restoring ? (seed as `0x${string}`) : undefined)}
      >
        {restoring ? "Restore demo wallet" : saved ? "Open demo wallet" : "Start demo wallet"}
      </Button>
      <button
        onClick={() => setRestoring((value) => !value)}
        className="mt-2 h-9 w-full rounded-full text-xs font-medium text-muted transition-colors hover:text-alabaster"
      >
        {restoring ? "Back" : "I have a saved seed"}
      </button>
    </Modal>
  );
}
