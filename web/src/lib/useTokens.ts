import { useCallback, useEffect, useMemo, useState } from "react";
import { KNOWN_TOKENS, NATIVE_ID, type Token } from "./tokens";

const STORAGE_KEY = "qday.tokens.v1";

interface Saved {
  /** Token ids the user chose to show. ETH is always shown. */
  selected: string[];
  /** Tokens the user added by address. */
  custom: Token[];
}

const DEFAULTS: Saved = { selected: KNOWN_TOKENS.map((t) => t.id), custom: [] };

function load(): Saved {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULTS;
    const parsed = JSON.parse(raw) as Partial<Saved>;
    return {
      selected: Array.isArray(parsed.selected) ? parsed.selected : DEFAULTS.selected,
      custom: Array.isArray(parsed.custom) ? parsed.custom.map((t) => ({ ...t, custom: true })) : [],
    };
  } catch {
    return DEFAULTS;
  }
}

function save(value: Saved): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(value));
  } catch {
    // Storage may be blocked; the choice just won't survive a reload.
  }
}

/** The token list: known tokens plus the user's own, and which ones the wallet shows. */
export function useTokens() {
  const [saved, setSaved] = useState<Saved>(load);
  useEffect(() => save(saved), [saved]);

  const all = useMemo(() => [...KNOWN_TOKENS, ...saved.custom], [saved.custom]);
  const byId = useMemo(() => new Map(all.map((t) => [t.id, t])), [all]);
  const isSelected = useCallback((id: string) => id === NATIVE_ID || saved.selected.includes(id), [saved.selected]);
  const selected = useMemo(() => all.filter((t) => isSelected(t.id)), [all, isSelected]);

  const toggle = useCallback((id: string) => {
    if (id === NATIVE_ID) return;
    setSaved((s) => ({
      ...s,
      selected: s.selected.includes(id) ? s.selected.filter((x) => x !== id) : [...s.selected, id],
    }));
  }, []);

  const addCustom = useCallback((token: Omit<Token, "id" | "custom">) => {
    if (!token.address) return;
    const id = token.address.toLowerCase();
    setSaved((s) => {
      if (KNOWN_TOKENS.some((t) => t.id === id) || s.custom.some((t) => t.id === id)) {
        return { ...s, selected: s.selected.includes(id) ? s.selected : [...s.selected, id] };
      }
      return { custom: [...s.custom, { ...token, id, custom: true }], selected: [...s.selected, id] };
    });
  }, []);

  const removeCustom = useCallback((id: string) => {
    setSaved((s) => ({ custom: s.custom.filter((t) => t.id !== id), selected: s.selected.filter((x) => x !== id) }));
  }, []);

  return { all, byId, selected, isSelected, toggle, addCustom, removeCustom };
}

export type TokenList = ReturnType<typeof useTokens>;
