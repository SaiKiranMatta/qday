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
