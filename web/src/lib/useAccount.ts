import type { DeviceStatus } from "@qday/device";
import { useCallback, useEffect, useRef, useState } from "react";
import { ActivityScanner } from "./activity";
import { type OnChainAccount, RPC_URL, accountAddress, readAccount } from "./chain";
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

  const root = status?.root ?? null;
  const pubSeed = status?.pubSeed ?? null;
  const tokenKey = tokens.map((t) => t.id).join(",");

  useEffect(() => {
    if (status) {
      console.info("[qday] device", {
        publicKey: status.root,
        publicSeed: status.pubSeed,
        nextSignature: status.nextIndex,
        keySet: status.tree,
      });
    }
  }, [status]);

  const refresh = useCallback(async () => {
    if (!root || !pubSeed) return;
    try {
      const address = await accountAddress(root, pubSeed);
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
  }, [root, pubSeed, tokenKey]);

  useEffect(() => {
    if (!root) {
      setOnChain(null);
      setChain("loading");
      return;
    }
    void refresh();
    const timer = setInterval(() => void refresh(), POLL_MS);
    return () => clearInterval(timer);
  }, [root, refresh]);

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
