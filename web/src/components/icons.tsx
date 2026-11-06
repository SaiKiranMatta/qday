import {
  ArrowDownLeft,
  ArrowLeftRight,
  ArrowUpRight,
  Check,
  ChevronDown,
  Copy,
  Cpu,
  KeyRound,
  Lock,
  type LucideIcon,
  Plus,
  RefreshCw,
  Unplug,
  Usb,
  Wallet,
  X,
} from "lucide-react";

/** Every icon in the app, from one set (Lucide), so they share a grid and stroke. */
export const ICONS = {
  wallet: Wallet,
  activity: ArrowLeftRight,
  device: Cpu,
  send: ArrowUpRight,
  receive: ArrowDownLeft,
  created: Plus,
  refresh: RefreshCw,
  refused: X,
  external: ArrowUpRight,
  chevron: ChevronDown,
  copy: Copy,
  check: Check,
  lock: Lock,
  disconnect: Unplug,
  usb: Usb,
  key: KeyRound,
} satisfies Record<string, LucideIcon>;

export type IconName = keyof typeof ICONS;

/** An icon at the app's stroke weight. Size it with a `size-*` class; it inherits the text colour. */
export function Icon({ name, className = "size-4" }: { name: IconName; className?: string }) {
  const Glyph = ICONS[name];
  return <Glyph className={`shrink-0 ${className}`} strokeWidth={1.75} aria-hidden />;
}
