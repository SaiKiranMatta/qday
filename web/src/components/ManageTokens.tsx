import { useState } from "react";
import { isAddress } from "../lib/tokens";
import type { TokenList } from "../lib/useTokens";
import { Modal } from "./Modal";
import { TokenLogo } from "./TokenLogo";
import { Button } from "./ui";

/** Pick which tokens the wallet shows, and add your own by contract address. */
export function ManageTokens({ tokens, onClose }: { tokens: TokenList; onClose: () => void }) {
  const [address, setAddress] = useState("");
  const [symbol, setSymbol] = useState("");
  const [decimals, setDecimals] = useState("18");
  const valid = isAddress(address) && symbol.trim().length > 0 && /^\d{1,2}$/.test(decimals);

  const add = () => {
    if (!valid || !isAddress(address)) return;
    tokens.addCustom({ address, symbol: symbol.trim().toUpperCase(), name: symbol.trim().toUpperCase(), decimals: Number(decimals) });
    setAddress("");
    setSymbol("");
    setDecimals("18");
  };

  return (
    <Modal title="Manage tokens" onClose={onClose} closeLabel="Done">
        <div className="max-h-72 space-y-1 overflow-auto overscroll-contain">
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
                    {token.custom && " · Added by you"}
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
          <h3 className="text-sm font-medium">Add a token</h3>
          <p className="mt-1 text-xs text-muted">Any ERC-20 on this devnet. Your QDay shows unknown tokens with a warning.</p>
          <label htmlFor="token-address" className="mt-4 block text-xs text-muted">
            Contract address
          </label>
          <input
            id="token-address"
            value={address}
            onChange={(e) => setAddress(e.target.value.trim())}
            placeholder="0x…"
            autoComplete="off"
            spellCheck={false}
            className="mt-1.5 w-full rounded-xl bg-surface-2 px-3 py-2.5 font-mono text-xs outline-none placeholder:text-muted focus:ring-1 focus:ring-alabaster/25"
          />
          {address && !isAddress(address) && (
            <div className="mt-1.5 text-xs text-bad">Enter a full 0x address of 42 characters, with its capitals unchanged.</div>
          )}
          <div className="mt-3 flex gap-2">
            <div className="min-w-0 flex-1">
              <label htmlFor="token-symbol" className="block text-xs text-muted">
                Symbol
              </label>
              <input
                id="token-symbol"
                value={symbol}
                onChange={(e) => setSymbol(e.target.value)}
                placeholder="DAI"
                autoComplete="off"
                className="mt-1.5 w-full rounded-xl bg-surface-2 px-3 py-2.5 text-sm outline-none placeholder:text-muted focus:ring-1 focus:ring-alabaster/25"
              />
            </div>
            <div className="w-24">
              <label htmlFor="token-decimals" className="block text-xs text-muted">
                Decimals
              </label>
              <input
                id="token-decimals"
                value={decimals}
                onChange={(e) => setDecimals(e.target.value.trim())}
                inputMode="numeric"
                autoComplete="off"
                className="mt-1.5 w-full rounded-xl bg-surface-2 px-3 py-2.5 text-sm tabular-nums outline-none placeholder:text-muted focus:ring-1 focus:ring-alabaster/25"
              />
            </div>
          </div>
          <Button className="mt-4 w-full" variant="secondary" disabled={!valid} onClick={add}>
            Add token
          </Button>
        </div>
    </Modal>
  );
}
