import type { DeviceStatus } from "@qday/device";
import { useCallback, useEffect, useRef, useState } from "react";
import { ActivityScanner } from "./activity";
import { type OnChainAccount, RPC_URL, accountAddress, readAccount } from "./chain";
import { planMlAccount } from "./mlAccount";
import { NATIVE_ID, type Token } from "./tokens";
import { type AccountView, type ActivityItem, EMPTY_ACCOUNT } from "./wallet";

const POLL_MS = 6_000; // one devnet block

export type ChainState = "loading" | "live" | "error";

/** The account behind a connected device: its address comes from the factory, its state from the chain. */
export function useAccount(status: DeviceStatus | null, tokens: Token[]) {
  const [onChain, setOnChain] = useState<OnChainAccount | null>(null);
  const [chain, setChain] = useState<ChainState>("loading");
  const [error, setError] = useState<string | null>(null);
  const [activity, setActivity] = useState<ActivityItem[]>([]);
  const [activityReady, setActivityReady] = useState(false);
  const scanner = useRef<{ key: string; scan: ActivityScanner } | null>(null);

  // Stands for the device's keys, so the account is looked up again only when they change.
  const keyId = status ? (status.scheme === "ml-dsa-44" ? status.publicKey : `${status.root}|${status.pubSeed}`) : null;
  const tokenKey = tokens.map((t) => t.id).join(",");

  useEffect(() => {
    if (status) console.info("[qday] device", status);
  }, [status]);

  const refresh = useCallback(async () => {
    if (!status) return;
    try {
      const address =
        status.scheme === "ml-dsa-44"
          ? (await planMlAccount(status.publicKey)).address
          : await accountAddress(status.root, status.pubSeed);
      const next = await readAccount(address, tokens);
      console.info("[qday] account", { rpc: RPC_URL, ...next, balanceWei: next.balanceWei.toString() });
      setOnChain(next);
      setChain("live");
      setError(null);
      const key = `${address}|${tokens.map((t) => t.id).join(",")}`;
      if (scanner.current?.key !== key) {
        scanner.current = { key, scan: new ActivityScanner(address, tokens) };
        setActivityReady(false);
      }
      try {
        setActivity(await scanner.current.scan.update());
        setActivityReady(true);
      } catch (e) {
        console.warn("[qday] activity scan failed; will retry", e);
      }
    } catch (e) {
      console.error("[qday] chain read failed", { rpc: RPC_URL, error: e });
      setError(e instanceof Error ? e.message : String(e));
      setChain("error");
    }
    // tokenKey stands in for tokens, which is a new array each render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [keyId, tokenKey]);

  useEffect(() => {
    if (!keyId) {
      setOnChain(null);
      setChain("loading");
      return;
    }
    void refresh();
    const timer = setInterval(() => void refresh(), POLL_MS);
    return () => clearInterval(timer);
  }, [keyId, refresh]);

  const account: AccountView = onChain
    ? {
        name: "Account 1",
        address: onChain.address,
        balance: onChain.balances[NATIVE_ID] ?? "0",
        balances: onChain.balances,
        activity,
        activityReady,
      }
    : EMPTY_ACCOUNT;

  return { account, onChain, chain, error, refresh };
}
