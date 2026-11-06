import QRCode from "qrcode";
import { useEffect, useState } from "react";
import { shortHex } from "../lib/format";
import type { SetupState } from "../lib/useStealth";
import { Modal } from "./Modal";
import { Button } from "./ui";

/** Your address as a QR code, or your private address, each with a copy button. */
export function ReceiveModal({
  address,
  privateAddress,
  privateSetup,
  demo,
  onEnablePrivate,
  onClose,
}: {
  address: `0x${string}`;
  privateAddress: `0x${string}` | null;
  privateSetup: SetupState;
  demo: boolean;
  /** Null when there's no account yet to set up private payments for. */
  onEnablePrivate: (() => void) | null;
  onClose: () => void;
}) {
  const [tab, setTab] = useState<"address" | "private">("address");
  const [qr, setQr] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    QRCode.toDataURL(address, { margin: 1, width: 240, color: { dark: "#0a0a0a", light: "#e5e4e2" } })
      .then(setQr)
      .catch(() => setQr(null));
  }, [address]);

  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard blocked; the address is selectable.
    }
  };

  return (
    <Modal title="Receive" onClose={onClose}>
      <div role="tablist" aria-label="Address type" className="mb-5 flex rounded-full bg-surface-2 p-1">
        {(["address", "private"] as const).map((id) => (
          <button
            key={id}
            role="tab"
            aria-selected={tab === id}
            onClick={() => {
              setTab(id);
              setCopied(false);
            }}
            className={`h-8 flex-1 rounded-full text-xs font-medium transition-colors ${
              tab === id ? "bg-surface text-alabaster" : "text-muted hover:text-alabaster"
            }`}
          >
            {id === "address" ? "Address" : "Private"}
          </button>
        ))}
      </div>

      {tab === "address" ? (
        <div className="flex flex-col items-center">
          {qr ? (
            <img src={qr} alt="Your address as a QR code" className="size-60 rounded-2xl" />
          ) : (
            <div className="size-60 rounded-2xl bg-surface-2" />
          )}
          <p className="mt-5 break-all text-center font-mono text-xs leading-relaxed text-alabaster/90">{address}</p>
          <p className="mt-2 text-center text-xs text-muted">Send only devnet ETH and tokens to this address.</p>
          <Button className="mt-5 w-full" onClick={() => void copy(address)}>
            {copied ? "Copied" : "Copy address"}
          </Button>
        </div>
      ) : privateAddress ? (
        <div>
          <p className="text-sm leading-relaxed text-alabaster/90">
            Share this once. Anyone can pay it as often as they like, and each payment lands in a new place that only you can
            find. They need no message from you first.
          </p>
          <div className="mt-4 rounded-2xl bg-surface-2 px-4 py-3 font-mono text-xs text-alabaster/90">
            {shortHex(privateAddress, 10, 8)}
          </div>
          <p className="mt-2 text-xs text-muted">
            It's 4,160 bytes, too long for a QR code, so copy it. It takes ETH only, on this devnet.
          </p>
          <Button className="mt-5 w-full" onClick={() => void copy(privateAddress)}>
            {copied ? "Copied" : "Copy private address"}
          </Button>
        </div>
      ) : (
        <div>
          <p className="text-sm leading-relaxed text-muted">
            {demo
              ? "Turns on a private address for this demo wallet."
              : "Your QDay asks you to share the private address, then the key that finds payments, which can't spend them."}
          </p>
          {privateSetup.kind === "error" && (
            <p role="alert" className="mt-3 text-xs text-bad">
              {privateSetup.message}
            </p>
          )}
          <Button
            className="mt-5 w-full"
            disabled={!onEnablePrivate || privateSetup.kind === "sharing"}
            onClick={() => onEnablePrivate?.()}
          >
            {privateSetup.kind === "sharing" ? "Setting up…" : demo ? "Set up" : "Set up on your QDay"}
          </Button>
          {!onEnablePrivate && <p className="mt-2 text-center text-xs text-muted">Create your account first.</p>}
        </div>
      )}
    </Modal>
  );
}
