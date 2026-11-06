import { useEffect, useRef, useState } from "react";
import type { Token } from "../lib/tokens";
import { TokenLogo } from "./TokenLogo";

/** The token chip in the amount field. Opens a list of the account's tokens with their balances. */
export function TokenPicker({
  tokens,
  selected,
  balances,
  onSelect,
}: {
  tokens: Token[];
  selected: Token;
  /** Formatted balance per token id. */
  balances: Record<string, string>;
  onSelect: (token: Token) => void;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (!open) return;
    const outside = (e: PointerEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [open]);

  const choose = (token: Token) => {
    setOpen(false);
    button.current?.focus();
    if (token.id !== selected.id) onSelect(token);
  };

  return (
    <div
      ref={root}
      className="relative shrink-0"
      onKeyDown={(e) => {
        // Escape closes the list, not the dialog around it.
        if (e.key === "Escape" && open) {
          e.stopPropagation();
          setOpen(false);
          button.current?.focus();
        }
      }}
    >
      <button
        ref={button}
        type="button"
        onClick={() => setOpen((o) => !o)}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-label={`Token: ${selected.symbol}. Change token`}
        className="flex items-center gap-2 rounded-full bg-surface py-1 pr-2.5 pl-1 text-sm font-medium transition-colors hover:bg-onyx/60"
      >
        <TokenLogo token={selected} size={24} />
        {selected.symbol}
        <svg viewBox="0 0 16 16" className={`size-3.5 text-muted transition-transform ${open ? "rotate-180" : ""}`} fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden>
          <path d="m4 6 4 4 4-4" />
        </svg>
      </button>

      {open && (
        <ul
          role="listbox"
          aria-label="Token to send"
          className="absolute top-full right-0 z-20 mt-2 w-64 rounded-2xl bg-surface p-1.5 shadow-2xl shadow-onyx ring-1 ring-line"
        >
          {tokens.map((token, i) => {
            const active = token.id === selected.id;
            return (
              <li key={token.id} role="presentation">
                <button
                  type="button"
                  role="option"
                  aria-selected={active}
                  autoFocus={active || (i === 0 && !tokens.some((t) => t.id === selected.id))}
                  onClick={() => choose(token)}
                  onKeyDown={(e) => {
                    const items = e.currentTarget.closest("ul")?.querySelectorAll<HTMLButtonElement>("[role=option]");
                    if (!items) return;
                    const at = [...items].indexOf(e.currentTarget);
                    if (e.key === "ArrowDown") items[(at + 1) % items.length]?.focus();
                    else if (e.key === "ArrowUp") items[(at - 1 + items.length) % items.length]?.focus();
                    else return;
                    e.preventDefault();
                  }}
                  className={`flex w-full items-center gap-3 rounded-xl px-2.5 py-2 text-left transition-colors hover:bg-surface-2 focus-visible:bg-surface-2 ${active ? "bg-surface-2" : ""}`}
                >
                  <TokenLogo token={token} size={28} />
                  <span className="min-w-0 flex-1">
                    <span className="block text-sm font-medium">{token.symbol}</span>
                    <span className="block truncate text-xs text-muted">{token.name}</span>
                  </span>
                  <span className="text-right text-xs tabular-nums text-alabaster/80">{balances[token.id] ?? "0"}</span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
