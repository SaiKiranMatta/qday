import QRCode from "qrcode";
import { useEffect, useState } from "react";
import { Modal } from "./Modal";
import { Button } from "./ui";

/** Your address as a QR code, with a copy button. */
export function ReceiveModal({ address, onClose }: { address: `0x${string}`; onClose: () => void }) {
  const [qr, setQr] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    QRCode.toDataURL(address, { margin: 1, width: 240, color: { dark: "#0a0a0a", light: "#e5e4e2" } })
      .then(setQr)
      .catch(() => setQr(null));
  }, [address]);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(address);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard blocked; the address is selectable.
    }
  };

  return (
    <Modal title="Receive" onClose={onClose}>
      <div className="flex flex-col items-center">
        {qr ? (
          <img src={qr} alt="Your address as a QR code" className="size-60 rounded-2xl" />
        ) : (
          <div className="size-60 rounded-2xl bg-surface-2" />
        )}
        <p className="mt-5 break-all text-center font-mono text-xs leading-relaxed text-alabaster/90">{address}</p>
        <p className="mt-2 text-center text-xs text-muted">Send only devnet ETH and tokens to this address.</p>
        <Button className="mt-5 w-full" onClick={copy}>
          {copied ? "Copied" : "Copy address"}
        </Button>
      </div>
    </Modal>
  );
}
