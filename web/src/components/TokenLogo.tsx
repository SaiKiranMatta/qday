import type { Token } from "../lib/tokens";

/** A token's logo, or a lettered badge when it has none. */
export function TokenLogo({ token, size = 36 }: { token: Pick<Token, "symbol" | "logo"> | undefined; size?: number }) {
  const style = { width: size, height: size };
  if (token?.logo) return <img src={token.logo} alt="" style={style} className="shrink-0 rounded-full" />;
  return (
    <span
      style={{ ...style, fontSize: Math.max(size * 0.3, 9) }}
      className="grid shrink-0 place-items-center rounded-full bg-slate/40 font-semibold text-alabaster"
    >
      {(token?.symbol ?? "?").slice(0, 3)}
    </span>
  );
}
