import { useEffect, useState } from "react";
import { isAddress } from "../lib/tokens";
import type { TokenList } from "../lib/useTokens";
import { TokenLogo } from "./TokenLogo";
import { Button } from "./ui";

/** Pick which tokens the wallet shows, and add your own by contract address. */
export function ManageTokens({ tokens, onClose }: { tokens: TokenList; onClose: () => void }) {
  const [address, setAddress] = useState("");
  const [symbol, setSymbol] = useState("");
  const [decimals, setDecimals] = useState("18");
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const valid = isAddress(address) && symbol.trim().length > 0 && /^\d{1,2}$/.test(decimals);

  const add = () => {
    if (!valid || !isAddress(address)) return;
    tokens.addCustom({ address, symbol: symbol.trim().toUpperCase(), name: symbol.trim().toUpperCase(), decimals: Number(decimals) });
    setAddress("");
    setSymbol("");
    setDecimals("18");
  };

  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-onyx/70 p-4 backdrop-blur-sm" onClick={onClose}>
      <div className="w-full max-w-md rounded-3xl bg-surface p-6" onClick={(e) => e.stopPropagation()}>
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold">Manage tokens</h2>
          <button onClick={onClose} className="text-sm text-muted hover:text-alabaster">
            Done
          </button>
        </div>

        <div className="mt-4 max-h-72 space-y-1 overflow-auto">
          {tokens.all.map((token) => {
            const locked = token.address === null;
            return (
              <label
                key={token.id}
                className={`flex items-center gap-3 rounded-xl px-3 py-2.5 ${locked ? "" : "cursor-pointer hover:bg-surface-2"}`}
              >
                <TokenLogo token={token} size={32} />
                <div className="min-w-0 flex-1">
                  <div className="text-sm font-medium">{token.name}</div>
                  <div className="text-xs text-muted">
                    {token.symbol}
                    {token.custom && " · added by you"}
                  </div>
                </div>
                {token.custom && (
                  <button
                    onClick={(e) => {
                      e.preventDefault();
                      tokens.removeCustom(token.id);
                    }}
                    className="text-xs text-muted hover:text-bad"
                  >
                    Remove
                  </button>
                )}
                <input
                  type="checkbox"
                  className="size-4 accent-slate"
                  checked={tokens.isSelected(token.id)}
                  disabled={locked}
                  onChange={() => tokens.toggle(token.id)}
                />
              </label>
            );
          })}
        </div>

        <div className="mt-5 border-t border-line/60 pt-5">
          <div className="text-sm font-medium">Add a token</div>
          <input
            value={address}
            onChange={(e) => setAddress(e.target.value.trim())}
            placeholder="Contract address (0x…)"
            className="mt-3 w-full rounded-xl bg-surface-2 px-3 py-2.5 font-mono text-xs outline-none placeholder:text-muted focus:ring-1 focus:ring-alabaster/25"
          />
          <div className="mt-2 flex gap-2">
            <input
              value={symbol}
              onChange={(e) => setSymbol(e.target.value)}
              placeholder="Symbol"
              className="min-w-0 flex-1 rounded-xl bg-surface-2 px-3 py-2.5 text-sm outline-none placeholder:text-muted focus:ring-1 focus:ring-alabaster/25"
            />
            <input
              value={decimals}
              onChange={(e) => setDecimals(e.target.value.trim())}
              placeholder="Decimals"
              className="w-24 rounded-xl bg-surface-2 px-3 py-2.5 text-sm outline-none placeholder:text-muted focus:ring-1 focus:ring-alabaster/25"
            />
          </div>
          {address && !isAddress(address) && <div className="mt-2 text-xs text-bad">That isn't a contract address.</div>}
          <Button className="mt-3 w-full" variant="secondary" disabled={!valid} onClick={add}>
            Add token
          </Button>
        </div>
      </div>
    </div>
  );
}
