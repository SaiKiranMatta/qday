import type { DeviceStatus } from "@qday/device";
import { SCHEMES } from "./schemes";

/** How many more signatures the device can make before it needs a key refresh. Null when unlimited. */
export interface Capacity {
  left: number | null;
  /** Signatures the current keys allow in all, the reserved ones aside. Null when unlimited. */
  total: number | null;
  /** True when the user should refresh keys soon. */
  low: boolean;
  /** True when signing is blocked until a refresh. */
  empty: boolean;
}

const LOW_AT = 50;

/** Turns scheme details into the one thing people need to know. Screens use this, not the scheme. */
export function capacity(status: DeviceStatus): Capacity {
  const scheme = SCHEMES[status.scheme];
  if (!scheme.stateful || scheme.keysPerTree === undefined) return { left: null, total: null, low: false, empty: false };
  const usable = scheme.keysPerTree - (scheme.reservedPerTree ?? 0);
  const left = Math.max(usable - status.nextIndex, 0);
  return { left, total: usable, low: left <= LOW_AT, empty: left === 0 };
}
