import type { DeviceStatus } from "@qday/device";
import { useCallback, useEffect, useState } from "react";
import { RPC_URL, readAccount } from "./chain";
import { type Address, NATIVE_ID, type Token } from "./tokens";
import { type AccountView, EMPTY_ACCOUNT } from "./wallet";

const STORAGE_KEY = "qday.accounts.v1";
const POLL_MS = 6_000; // one devnet block

/** Account address linked to each device key, so a device finds its account again. */
type Links = Record<string, { name: string; address: Address }>;

function loadLinks(): Links {
  try {
    return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "{}") as Links;
  } catch {
    return {};
  }
}

function saveLinks(links: Links): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(links));
  } catch {
    // Blocked storage: the link lasts until reload.
  }
}

export type ChainState = "unlinked" | "loading" | "live" | "error";

/** The account behind a connected device: its linked address and live balances from the chain. */
export function useAccount(status: DeviceStatus | null, tokens: Token[]) {
  const [links, setLinks] = useState<Links>(loadLinks);
  const [account, setAccount] = useState<AccountView>(EMPTY_ACCOUNT);
  const [chain, setChain] = useState<ChainState>("unlinked");
  const [warning, setWarning] = useState<string | null>(null);

  const key = status?.root.toLowerCase() ?? null;
  const link = key ? links[key] : undefined;

  useEffect(() => {
    if (status) {
      console.info("[qday] device", {
        publicKey: status.root,
        publicSeed: status.pubSeed,
        nextSignature: status.nextIndex,
        keySet: status.tree,
        nextKeySet: status.next,
      });
    }
  }, [status]);

  const linkAddress = useCallback(
    (address: Address, name = "Account 1") => {
      if (!key) return;
      setLinks((current) => {
        const next = { ...current, [key]: { name, address } };
        saveLinks(next);
        return next;
      });
    },
    [key],
  );

  const unlink = useCallback(() => {
    if (!key) return;
    setLinks((current) => {
      const { [key]: _, ...rest } = current;
      saveLinks(rest);
      return rest;
    });
  }, [key]);

  const tokenKey = tokens.map((t) => t.id).join(",");

  const refresh = useCallback(async () => {
    if (!link || !status) return;
    try {
      const onChain = await readAccount(link.address, tokens);
      console.info("[qday] account", { address: link.address, rpc: RPC_URL, ...onChain });
      let problem: string | null = null;
      if (!onChain.isContract) problem = "No account contract at this address on the devnet. It may not be deployed yet, or the devnet was reset.";
      else if (onChain.root && onChain.root.toLowerCase() !== status.root.toLowerCase())
        problem = "This account belongs to different keys than the connected device.";
      if (problem) console.warn("[qday]", problem, { device: status.root, chain: onChain.root });
      setWarning(problem);
      setAccount({
        name: link.name,
        address: link.address,
        balance: onChain.balances[NATIVE_ID] ?? "0",
        balances: onChain.balances,
        activity: [],
      });
      setChain("live");
    } catch (error) {
      console.error("[qday] chain read failed", { address: link.address, rpc: RPC_URL, error });
      setChain("error");
    }
    // tokenKey stands in for tokens, which is a new array each render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [link?.address, link?.name, status?.root, tokenKey]);

  useEffect(() => {
    if (!link) {
      setAccount(EMPTY_ACCOUNT);
      setChain("unlinked");
      setWarning(null);
      return;
    }
    setChain((c) => (c === "live" ? c : "loading"));
    void refresh();
    const timer = setInterval(() => void refresh(), POLL_MS);
    return () => clearInterval(timer);
  }, [link, refresh]);

  return { account, chain, warning, linkAddress, unlink, refresh };
}
