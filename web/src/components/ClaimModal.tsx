import { type ReactNode, useState } from "react";
import { exactEth, shortHex } from "../lib/format";
import { SPEND_FEE } from "../lib/stealth";
import { isAddress } from "../lib/tokens";
import type { ClaimStep, PrivateNote } from "../lib/useStealth";
import { Modal } from "./Modal";
import { Button } from "./ui";

const TITLES: Record<ClaimStep["kind"], string> = {
  edit: "Claim payment",
  approve: "Approve on QDay",
  relaying: "Claiming",
  done: "Claimed",
};

/** Sends a whole private note to an address. The fee comes out of the note, so no ETH is needed first. */
export function ClaimModal({
  note,
  ownAddress,
  step,
  onClaim,
  onClose,
}: {
  note: PrivateNote;
  ownAddress: `0x${string}`;
  step: ClaimStep;
  onClaim: (to: `0x${string}`) => void;
  onClose: () => void;
}) {
  const [to, setTo] = useState<string>(ownAddress);
  const valid = isAddress(to);
  const enough = note.valueWei > SPEND_FEE;
  const busy = step.kind === "approve" || step.kind === "relaying";
  const toOwn = to.toLowerCase() === ownAddress.toLowerCase();

  return (
    <Modal title={TITLES[step.kind]} onClose={onClose} locked={busy}>
      <dl className="divide-y divide-line/60 rounded-2xl bg-surface-2 px-4 text-sm">
        <Line label="Payment">{exactEth(note.valueWei)} ETH</Line>
        <Line label="Fee">{exactEth(SPEND_FEE)} ETH</Line>
        <Line label="You get" strong>
          {enough ? `${exactEth(note.valueWei - SPEND_FEE)} ETH` : "Nothing"}
        </Line>
      </dl>

      {step.kind === "edit" && (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (valid && enough) onClaim(to as `0x${string}`);
          }}
        >
          <label htmlFor="claim-to" className="mt-5 block text-xs text-muted">
            Send to
          </label>
          <input
            id="claim-to"
            value={to}
            onChange={(e) => setTo(e.target.value.trim())}
            autoComplete="off"
            spellCheck={false}
            className="mt-2 w-full rounded-2xl bg-surface-2 px-4 py-3 font-mono text-xs outline-none focus:ring-1 focus:ring-alabaster/25"
          />
          <p className="mt-2 min-h-5 text-xs text-muted">
            {!valid
              ? "Enter a full 0x address, 42 characters."
              : !enough
                ? "This payment is too small to cover the fee."
                : toOwn
                  ? "Claiming to this account shows on chain that the payment was yours. Use a fresh address to keep it apart."
                  : "The whole payment goes here, less the fee."}
          </p>
          {step.error && (
            <p role="alert" className="mt-2 rounded-xl bg-bad/10 px-3.5 py-3 text-xs text-bad">
              {step.error}
            </p>
          )}
          <Button type="submit" className="mt-4 w-full" disabled={!valid || !enough}>
            Continue on QDay
          </Button>
        </form>
      )}

      {busy && (
        <p role="status" aria-live="polite" className="mt-5 rounded-2xl bg-surface-2 px-4 py-3.5 text-sm font-medium">
          {step.kind === "approve" ? "Check the address, then approve on your QDay." : "Submitting the claim to the devnet."}
        </p>
      )}

      {step.kind === "done" && (
        <div>
          <p role="status" className="mt-5 text-sm text-ok">
            Confirmed in block {step.block.toLocaleString("en-US")}, transaction {shortHex(step.hash, 8, 6)}.
          </p>
          <Button className="mt-5 w-full" onClick={onClose}>
            Done
          </Button>
        </div>
      )}
    </Modal>
  );
}

function Line({ label, strong, children }: { label: string; strong?: boolean; children: ReactNode }) {
  return (
    <div className="flex items-baseline justify-between gap-4 py-3">
      <dt className="text-muted">{label}</dt>
      <dd className={`tabular-nums ${strong ? "font-semibold" : ""}`}>{children}</dd>
    </div>
  );
}
