import { loadFrame } from "@qday/frame";
import { useEffect, useState } from "react";
import { SEND_TITLES, SendFlow } from "./components/SendFlow";
import { KNOWN_TOKENS, NATIVE_ID } from "./lib/tokens";
import type { Draft, SendStep } from "./lib/useSend";

const ACCOUNT = "0x8448b66248f3ac3a8ba418512b7c73d8585c5007";
const TO = "0x8943545177806ed17b9f23f0a21ee5948ecaa776";
const BALANCE = 999_070_940_000_000_000n;

/** Dev only (`?gallery`): every send state side by side, without a device or a chain. */
export function Gallery() {
  const [draft, setDraft] = useState<Draft | null>(null);
  const [fee, setFee] = useState<bigint | null>(null);

  useEffect(() => {
    void loadFrame().then((frame) => {
      const raw = frame.transferTx(ACCOUNT, 3n, 2, TO, 10_000_000_000_000_000n);
      const tx = frame.describe(raw);
      setDraft({ raw, tx });
      setFee(BigInt(tx.maxFeeWei));
    });
  }, []);

  if (!draft) return <div className="p-10 text-muted">Loading the frame code…</div>;
  const states: { label: string; step: SendStep; to?: string; amount?: string }[] = [
    { label: "Empty", step: { kind: "edit" } },
    { label: "Filled", step: { kind: "edit" }, to: TO, amount: "0.01" },
    {
      label: "Too much",
      step: { kind: "edit", error: "Not enough ETH for this amount plus the network fee. Lower the amount, or use Max." },
      to: "0x1234",
      amount: "5",
    },
    { label: "Review", step: { kind: "review", draft } },
    { label: "Rejected", step: { kind: "review", draft, error: "Rejected on the device. Nothing was signed." } },
    { label: "Approve", step: { kind: "approve", draft } },
    { label: "Sending", step: { kind: "submitting", draft } },
    {
      label: "Done",
      step: { kind: "done", draft, block: 412, hash: "0x7508072f568cf40faa110f50274c5408ca24f3b525fc8b2a68b0cd811ad9852d" },
    },
  ];

  return (
    <div className="grid gap-6 p-8 [grid-template-columns:repeat(auto-fill,minmax(min(400px,100%),1fr))]">
      {states.map(({ label, step, to, amount }) => (
        <section key={label}>
          <div className="mb-2 text-xs text-muted">{label}</div>
          <div className="rounded-3xl bg-surface p-6">
            <h2 className="mb-5 text-lg font-semibold">{SEND_TITLES[step.kind]}</h2>
            <SendFlow
              step={step}
              eth={KNOWN_TOKENS.find((t) => t.id === NATIVE_ID)}
              balanceWei={BALANCE}
              feeWei={fee}
              ownAddress={ACCOUNT}
              initialTo={to}
              initialAmount={amount}
              onReview={() => {}}
              onConfirm={() => {}}
              onEdit={() => {}}
              onClose={() => {}}
            />
          </div>
        </section>
      ))}
    </div>
  );
}
