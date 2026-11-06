import type { Token } from "../lib/tokens";
import type { SendStep } from "../lib/useSend";
import { Modal } from "./Modal";
import { SEND_TITLES, SendFlow } from "./SendFlow";

/** Send ETH in a dialog. It can't be dismissed while the device or the network is working. */
export function SendModal(props: {
  step: SendStep;
  eth: Token | undefined;
  balanceWei: bigint;
  feeWei: bigint | null;
  ownAddress: string;
  onReview: (to: `0x${string}`, amountEth: string) => void;
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
