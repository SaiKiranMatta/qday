import type { DemoRequest } from "@qday/frame";
import type { ReactNode } from "react";
import { addressLines, exactEth, exactUnits } from "../lib/format";
import { Modal } from "./Modal";
import { Button } from "./ui";

interface Shown {
  title: string;
  amount: string | null;
  label: string;
  to: string | null;
  fee: string;
}

/** The request as a device's review screen would put it. */
function shown(request: DemoRequest): Shown {
  if (request.kind === "spend") {
    return { title: "Spend note", amount: "Whole note", label: "To", to: request.to, fee: `${exactEth(request.fee)} ETH` };
  }
  const { action, maxFeeWei } = request.tx;
  const fee = `Up to ${exactEth(BigInt(maxFeeWei))} ETH`;
  switch (action.kind) {
    case "transfer":
      return { title: "Send", amount: `${exactEth(BigInt(action.valueWei))} ETH`, label: "To", to: action.to, fee };
    case "stealthPay":
      return { title: "Private payment", amount: `${exactEth(BigInt(action.valueWei))} ETH`, label: "Pool", to: action.pool, fee };
    case "token": {
      const units = BigInt(action.amount);
      const amount = action.decimals === null ? `${units} units` : `${exactUnits(units, action.decimals)} ${action.symbol}`;
      return { title: "Send token", amount, label: "To", to: action.to, fee };
    }
    case "create":
      return { title: "Create account", amount: null, label: "Factory", to: action.factory, fee };
    case "rotate":
      return { title: "Rotate keys", amount: null, label: "To", to: null, fee };
    case "other":
      return { title: "Unknown call", amount: `${exactEth(BigInt(action.valueWei))} ETH`, label: "To", to: action.target, fee };
  }
}

/** Stands in for the device's review screen, so the demo goes through the same steps as the real thing. */
export function DemoApproval({ request, onDecide }: { request: DemoRequest; onDecide: (approved: boolean) => void }) {
  const { title, amount, label, to, fee } = shown(request);
  return (
    <Modal title="Demo device" onClose={() => onDecide(false)} locked>
      <div className="rounded-2xl bg-onyx px-4 py-4 ring-1 ring-line">
        <div className="text-xs font-medium text-muted">{title}</div>
        {amount && <div className="mt-1 text-3xl font-semibold tracking-tight tabular-nums">{amount}</div>}
        <dl className="mt-3 space-y-2 text-sm">
          {to && (
            <Line label={label}>
              <span className="font-mono text-[13px] leading-6 whitespace-nowrap tabular-nums">
                {addressLines(to)[0]}
                <br />
                {addressLines(to)[1]}
              </span>
            </Line>
          )}
          <Line label="Fee">{fee}</Line>
        </dl>
      </div>
      <p className="mt-3 text-xs leading-relaxed text-muted">
        On a real QDay this appears on the device, where this page can't change it, and it needs your passcode.
      </p>
      <div className="mt-4 flex gap-3">
        <Button variant="secondary" className="flex-1" onClick={() => onDecide(false)}>
          Reject
        </Button>
        <Button className="flex-1" onClick={() => onDecide(true)}>
          Approve
        </Button>
      </div>
    </Modal>
  );
}

function Line({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-baseline gap-4">
      <dt className="w-14 shrink-0 text-muted">{label}</dt>
      <dd className="min-w-0 flex-1 tabular-nums">{children}</dd>
    </div>
  );
}
