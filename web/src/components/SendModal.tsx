import type { Token } from "../lib/tokens";
import type { SendFees, SendStep } from "../lib/useSend";
import { Modal } from "./Modal";
import { SEND_TITLES, SendFlow } from "./SendFlow";

/** Send ETH or a token in a dialog. It can't be dismissed while the device or the network is working. */
export function SendModal(props: {
  step: SendStep;
  tokens: Token[];
  balances: Record<string, bigint>;
  fees: SendFees | null;
  ownAddress: string;
  onReview: (token: Token, to: `0x${string}`, amount: string) => void;
  onConfirm: () => void;
  onEdit: () => void;
  onClose: () => void;
}) {
  const busy = props.step.kind === "approve" || props.step.kind === "submitting";
  return (
    <Modal title={SEND_TITLES[props.step.kind]} onClose={props.onClose} locked={busy}>
      <SendFlow {...props} />
    </Modal>
  );
}
