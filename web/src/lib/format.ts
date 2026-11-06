/** 0x1234…cdef, keeping the start and end people actually compare. */
export function shortHex(hex: string, head = 8, tail = 4): string {
  if (hex.length <= head + tail + 1) return hex;
  return `${hex.slice(0, head)}…${hex.slice(-tail)}`;
}

/** #0067, as the device shows key numbers. */
export function keyNumber(index: number): string {
  return `#${index.toString().padStart(4, "0")}`;
}

export function groupThousands(value: number): string {
  return value.toLocaleString("en-US");
}

/**
 * An address in two lines of four-character groups, exactly as the device's review screen draws it,
 * so the two can be compared group by group.
 */
export function addressLines(address: string): [string, string] {
  const digits = address.toLowerCase().replace(/^0x/, "");
  const groups = (part: string) => part.match(/.{1,4}/g)?.join(" ") ?? part;
  return [`0x${groups(digits.slice(0, 20))}`, groups(digits.slice(20))];
}

/** An exact amount of a token with `decimals` places, trailing zeros trimmed, as the device shows it. */
export function exactUnits(value: bigint, decimals: number): string {
  const unit = 10n ** BigInt(decimals);
  const whole = (value / unit).toLocaleString("en-US");
  const fraction = decimals ? (value % unit).toString().padStart(decimals, "0").replace(/0+$/, "") : "";
  return fraction ? `${whole}.${fraction}` : whole;
}

/** An exact ETH amount (no rounding). */
export function exactEth(wei: bigint): string {
  return exactUnits(wei, 18);
}

/** Reads a typed amount in a token's units. Null unless it is a plain decimal with at most `decimals` places. */
export function parseAmount(text: string, decimals: number): bigint | null {
  const match = /^(\d*)(?:\.(\d*))?$/.exec(text);
  if (!match || text === "" || text === ".") return null;
  const [, whole = "", fraction = ""] = match;
  if (fraction.length > decimals) return null;
  return BigInt(whole || "0") * 10n ** BigInt(decimals) + BigInt(fraction.padEnd(decimals, "0") || "0");
}
