import { useState } from "react";
import { formatAmount } from "../lib/chain";
import type { Address } from "../lib/tokens";
import type { SetupStep } from "../lib/useCreateAccount";
import { Button, StatusDot } from "./ui";

/**
 * First run: the account's address is known before it exists. Fund it, then create it with one
 * transaction the device signs. The account pays for its own creation.
 */
export function SetupCard({
  address,
  balanceWei,
  step,
  onFund,
  onCreate,
}: {
  address: Address | null;
  balanceWei: bigint;
  step: SetupStep;
  onFund: () => void;
  onCreate: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const funded = balanceWei > 0n;
  const busy = step.kind === "funding" || step.kind === "approve" || step.kind === "submitting";

  const copy = async () => {
    if (!address) return;
    try {
      await navigator.clipboard.writeText(address);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // Clipboard blocked; the address is selectable.
    }
  };

  return (
    <div className="relative z-10 w-full">
      <h1 className="text-2xl font-semibold tracking-tight">Create your account</h1>
      <p className="mt-2 max-w-xl text-sm leading-relaxed text-alabaster/75">
        Your QDay already has an address. Add test ETH to it, then approve one transaction on the device to create the
        account. The account pays its own creation fee.
      </p>

      <div className="mt-5 grid gap-3 lg:grid-cols-2">
        <div className="rounded-2xl bg-onyx/70 p-4 backdrop-blur">
          <div className="flex items-center justify-between text-xs text-alabaster/60">
            <span>Step 1 · Add funds</span>
            <span>{funded ? `${formatAmount(balanceWei, 18)} ETH` : "No funds yet"}</span>
          </div>
          <div className="mt-2 flex items-center gap-2">
            <code className="min-w-0 flex-1 truncate font-mono text-xs" title={address ?? ""}>
              {address ?? "Finding your address…"}
            </code>
            <button onClick={copy} disabled={!address} className="shrink-0 text-xs text-alabaster/70 hover:text-alabaster">
              {copied ? "Copied" : "Copy"}
            </button>
          </div>
          <Button variant="secondary" className="mt-3 h-9 w-full" disabled={!address || busy} onClick={onFund}>
            {step.kind === "funding" ? "Sending test ETH…" : "Get test ETH"}
          </Button>
        </div>

        <div className="rounded-2xl bg-onyx/70 p-4 backdrop-blur">
          <div className="text-xs text-alabaster/60">Step 2 · Create account</div>
          <p className="mt-2 text-xs text-alabaster/80">
            {step.kind === "approve"
              ? "Check the address on your QDay, then approve “Create account”."
              : step.kind === "submitting"
                ? "Creating your account. This takes about one block."
                : funded
                  ? "One transaction, approved on your QDay."
                  : "Add funds first, so the account can pay for its creation."}
          </p>
          <Button className="mt-3 h-9 w-full" disabled={!funded || busy || !address} onClick={onCreate}>
            {step.kind === "approve" ? "Waiting for your QDay…" : step.kind === "submitting" ? "Creating…" : "Create account"}
          </Button>
        </div>
      </div>

      {step.kind === "error" && (
        <div className="mt-4 flex items-start gap-2 rounded-xl bg-bad/15 p-3 text-xs text-bad">
          <StatusDot tone="bad" />
          <span>{step.message}</span>
        </div>
      )}
    </div>
  );
}
