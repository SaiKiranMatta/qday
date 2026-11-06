import { useState } from "react";
import { type Page, Shell } from "./components/Shell";
import { ConnectPanel } from "./components/ConnectPanel";
import type { DeviceStatus } from "@qday/device";
import { useAccount } from "./lib/useAccount";
import { useCreateAccount } from "./lib/useCreateAccount";
import { useSend } from "./lib/useSend";
import { ReceiveModal } from "./components/ReceiveModal";
import { SendModal } from "./components/SendModal";
import { useDevice } from "./lib/useDevice";
import { useTokens } from "./lib/useTokens";
import { CAN_MINT_TEST_TOKENS, requestTestTokens } from "./lib/chain";
import { MOCK, MOCK_ACCOUNT } from "./lib/wallet";
import { ActivityPage, DevicePage, WalletPage } from "./pages";

export function App() {
  const { state, log, connect, refresh, disconnect, signFrame } = useDevice();

  if (state.kind === "connected") {
    return (
      <ConnectedApp status={state.status} log={log} onRefresh={refresh} onDisconnect={disconnect} signFrame={signFrame} />
    );
  }

  // No device yet: the same shell, with the connect panel where the wallet goes.
  return (
    <Shell
      page="wallet"
      onNavigate={() => {}}
      device={{ connected: false, connecting: state.kind === "connecting" }}
    >
      <ConnectPanel state={state} onConnect={connect} />
    </Shell>
  );
}

/** Everything after the device is connected: its account on the chain, setup, and the wallet screens. */
function ConnectedApp({
  status,
  log,
  onRefresh,
  onDisconnect,
  signFrame,
}: {
  status: DeviceStatus;
  log: string[];
  onRefresh: () => void;
  onDisconnect: () => void;
  signFrame: (raw: Uint8Array) => Promise<`0x${string}`>;
}) {
  const [page, setPage] = useState<Page>("wallet");
  const tokens = useTokens();
  const live = useAccount(MOCK ? null : status, tokens.selected);
  const setup = useCreateAccount(status, live.onChain?.address ?? null, signFrame, () => void live.refresh());
  const account = MOCK ? MOCK_ACCOUNT : live.account;
  const needsSetup = !MOCK && !live.onChain?.deployed;
  const [dialog, setDialog] = useState<"send" | "receive" | null>(null);
  const send = useSend(status, live.onChain?.deployed ? live.onChain : null, signFrame, () => void live.refresh());
  const closeDialog = () => {
    setDialog(null);
    send.reset();
  };

  return (
    <Shell page={page} onNavigate={setPage} device={{ connected: true, onDisconnect }}>
      {page === "wallet" && (
        <WalletPage
          account={account}
          tokens={tokens}
          chain={MOCK ? "live" : live.chain}
          error={MOCK ? null : live.error}
          setup={
            needsSetup
              ? {
                  address: live.onChain?.address ?? null,
                  balanceWei: live.onChain?.balanceWei ?? 0n,
                  step: setup.step,
                  onFund: () => void setup.fund(),
                  onCreate: () => void setup.create(),
                }
              : null
          }
          onSend={!MOCK && live.onChain?.deployed ? () => setDialog("send") : null}
          onReceive={account.address !== "0x" ? () => setDialog("receive") : null}
          onMintTokens={
            !MOCK && CAN_MINT_TEST_TOKENS && live.onChain
              ? async () => {
                  await requestTestTokens(live.onChain!.address, tokens.selected);
                  await live.refresh();
                }
              : null
          }
          onOpenActivity={() => setPage("activity")}
        />
      )}
      {dialog === "send" && (
        <SendModal
          step={send.step}
          tokens={tokens.selected}
          balances={live.onChain?.units ?? {}}
          fees={send.fees}
          ownAddress={account.address}
          onReview={(token, to, amount) => void send.review(token, to, amount)}
          onConfirm={() => void send.confirm()}
          onEdit={send.edit}
          onClose={closeDialog}
        />
      )}
      {dialog === "receive" && account.address !== "0x" && <ReceiveModal address={account.address} onClose={closeDialog} />}
      {page === "activity" && <ActivityPage account={account} tokens={tokens} />}
      {page === "device" && <DevicePage status={status} log={log} onRefresh={onRefresh} onDisconnect={onDisconnect} />}
    </Shell>
  );
}
