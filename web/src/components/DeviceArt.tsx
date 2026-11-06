import { Wordmark } from "./ui";

export type DeviceLight = "idle" | "busy" | "ok";

const LIGHT: Record<DeviceLight, string> = {
  idle: "bg-alabaster/40",
  busy: "bg-warn animate-pulse",
  ok: "bg-ok animate-breathe",
};

/**
 * A QDay in the brand's chrome: screen above, two buttons below. Small, it's a mark; large, its
 * screen says what the real one is doing, and its light breathes while it's connected.
 */
export function DeviceArt({ size = "small", light, caption }: { size?: "small" | "large"; light: DeviceLight; caption?: string }) {
  const large = size === "large";
  return (
    <span
      aria-hidden
      className={`chrome grain relative block shrink-0 overflow-hidden shadow-lg shadow-onyx/50 ${
        large ? "h-40 w-[124px] rounded-[28px]" : "h-[72px] w-14 rounded-2xl"
      }`}
    >
      <span className={`absolute z-10 bg-onyx/90 ${large ? "inset-x-3.5 top-3.5 h-[84px] rounded-2xl" : "inset-x-2 top-2 h-9 rounded-lg"}`}>
        <span className={`absolute rounded-full ${LIGHT[light]} ${large ? "top-3 left-3 size-2" : "top-1.5 left-1.5 size-1.5"}`} />
        {large && (
          <span className="absolute inset-x-3 bottom-3">
            <Wordmark className="block text-xl text-alabaster/90" />
            {caption && <span className="mt-1 block truncate text-[9px] font-medium text-muted">{caption}</span>}
          </span>
        )}
      </span>
      <span className={`absolute z-10 flex justify-between ${large ? "inset-x-7 bottom-5" : "inset-x-3.5 bottom-3"}`}>
        <span className={`rounded-full bg-onyx/60 ${large ? "size-4" : "size-2"}`} />
        <span className={`rounded-full bg-onyx/60 ${large ? "size-4" : "size-2"}`} />
      </span>
    </span>
  );
}
