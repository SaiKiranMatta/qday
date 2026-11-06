import { loadFrame } from "@qday/frame";
import { useEffect, useState } from "react";
import { SEND_TITLES, SendFlow } from "./components/SendFlow";
import { SendModal } from "./components/SendModal";
import { Button } from "./components/ui";
import { KNOWN_TOKENS, NATIVE_ID } from "./lib/tokens";
import type { Draft, SendFees, SendStep } from "./lib/useSend";

const ACCOUNT = "0x8448b66248f3ac3a8ba418512b7c73d8585c5007";
const TO = "0x8943545177806ed17b9f23f0a21ee5948ecaa776";
const ETH = KNOWN_TOKENS.find((t) => t.id === NATIVE_ID)!;
const USDC = KNOWN_TOKENS.find((t) => t.symbol === "USDC")!;
const BALANCES: Record<string, bigint> = {
  [NATIVE_ID]: 999_070_940_000_000_000n,
  [USDC.id]: 2_500_000_000n,
  [KNOWN_TOKENS.find((t) => t.symbol === "WBTC")!.id]: 5_000_000n,
};

/** Dev only (`?gallery`): every send state side by side, without a device or a chain. */
export function Gallery() {
  const [draft, setDraft] = useState<Draft | null>(null);
  const [tokenDraft, setTokenDraft] = useState<Draft | null>(null);
  const [fees, setFees] = useState<SendFees | null>(null);
  const [playing, setPlaying] = useState<SendStep | null>(null);

  useEffect(() => {
    void loadFrame().then((frame) => {
      const raw = frame.transferTx(ACCOUNT, 3n, 2, TO, 10_000_000_000_000_000n);
      const tx = frame.describe(raw);
      setDraft({ raw, tx, token: ETH, stealth: false });
      const tokenRaw = frame.tokenTransferTx(ACCOUNT, 3n, 2, USDC.address!, TO, 125_500_000n);
      const tokenTx = frame.describe(tokenRaw);
      setTokenDraft({ raw: tokenRaw, tx: tokenTx, token: USDC, stealth: false });
      setFees({ eth: BigInt(tx.maxFeeWei), token: BigInt(tokenTx.maxFeeWei), stealth: BigInt(tx.maxFeeWei) });
    });
  }, []);

  if (!draft || !tokenDraft) return <div className="p-10 text-muted">Loading the frame code…</div>;
  const states: { label: string; step: SendStep; to?: string; amount?: string; token?: string }[] = [
    { label: "Empty", step: { kind: "edit" } },
    { label: "Filled", step: { kind: "edit" }, to: TO, amount: "0.01" },
    {
      label: "Too much",
      step: { kind: "edit", error: "Not enough ETH for this amount plus the network fee. Lower the amount, or use Max." },
      to: "0x1234",
      amount: "5",
    },
    { label: "Token", step: { kind: "edit" }, to: TO, amount: "125.5", token: USDC.id },
    { label: "Review", step: { kind: "review", draft } },
    { label: "Review token", step: { kind: "review", draft: tokenDraft } },
    { label: "Rejected", step: { kind: "review", draft, error: "Rejected on the device. Nothing was signed." } },
    { label: "Approve", step: { kind: "approve", draft } },
    { label: "Sending", step: { kind: "submitting", draft } },
    {
      label: "Done",
      step: { kind: "done", draft, block: 412, hash: "0x7508072f568cf40faa110f50274c5408ca24f3b525fc8b2a68b0cd811ad9852d" },
    },
    {
      label: "Done token",
      step: { kind: "done", draft: tokenDraft, block: 413, hash: "0x51ac2e0a7c1f6d3b9e8a4f2c7d0b5e3a9c6f1d8e4b7a2c5f0e9d3b6a8c1f4e7d" },
    },
  ];

  return (
    <div>
      <div className="flex items-center gap-4 px-8 pt-8">
        <Button onClick={() => setPlaying({ kind: "review", draft: tokenDraft })}>Play the send flow</Button>
        <span className="text-sm text-muted">Review, approve, sending and sent in the real dialog, on timers.</span>
      </div>
      {playing && (
        <SendModal
          step={playing}
          tokens={KNOWN_TOKENS}
          balances={BALANCES}
          fees={fees}
          ownAddress={ACCOUNT}
          onReview={() => {}}
          onConfirm={() => {
            const draft = tokenDraft;
            setPlaying({ kind: "approve", draft });
            setTimeout(() => setPlaying({ kind: "submitting", draft }), 2500);
            setTimeout(
              () => setPlaying({ kind: "done", draft, block: 1612, hash: "0x51ac2e0a7c1f6d3b9e8a4f2c7d0b5e3a9c6f1d8e4b7a2c5f0e9d3b6a8c1f4e7d" }),
              5500,
            );
          }}
          onEdit={() => setPlaying({ kind: "edit" })}
          onClose={() => setPlaying(null)}
        />
      )}
    <div className="grid gap-6 p-8 [grid-template-columns:repeat(auto-fill,minmax(min(400px,100%),1fr))]">
      {states.map(({ label, step, to, amount, token }) => (
        <section key={label}>
          <div className="mb-2 text-xs text-muted">{label}</div>
          <div className="rounded-3xl bg-surface p-6">
            <h2 className="mb-5 text-lg font-semibold">{SEND_TITLES[step.kind]}</h2>
            <SendFlow
              step={step}
              tokens={KNOWN_TOKENS}
              balances={BALANCES}
              fees={fees}
              ownAddress={ACCOUNT}
              initialTo={to}
              initialAmount={amount}
              initialToken={token}
              onReview={() => {}}
              onConfirm={() => {}}
              onEdit={() => {}}
              onClose={() => {}}
            />
          </div>
        </section>
      ))}
    </div>
    </div>
  );
}
