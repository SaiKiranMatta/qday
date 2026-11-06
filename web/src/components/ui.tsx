import type { ComponentProps, ReactNode } from "react";

/** The lowercase italic wordmark, in the brand's heavy oblique style. */
export function Wordmark({ className = "" }: { className?: string }) {
  return (
    <span className={`select-none font-black italic tracking-[-0.06em] lowercase leading-none ${className}`}>qday</span>
  );
}

type ButtonVariant = "primary" | "secondary" | "ghost" | "onyx";

const BUTTON: Record<ButtonVariant, string> = {
  primary: "bg-alabaster text-onyx hover:bg-white",
  secondary: "bg-surface-2 text-alabaster hover:bg-line",
  ghost: "text-muted hover:text-alabaster",
  onyx: "bg-onyx/85 text-alabaster backdrop-blur hover:bg-onyx",
};

/** Pill buttons, like the device's Approve and Reject. */
export function Button({
  variant = "primary",
  className = "",
  ...props
}: ComponentProps<"button"> & { variant?: ButtonVariant }) {
  return (
    <button
      className={`inline-flex h-11 items-center justify-center rounded-full px-6 text-sm font-semibold transition-colors disabled:cursor-not-allowed disabled:opacity-40 ${BUTTON[variant]} ${className}`}
      {...props}
    />
  );
}

/** A dark rounded card with a small grey label, like the device's tiles. */
export function Tile({ label, children, className = "" }: { label: string; children: ReactNode; className?: string }) {
  return (
    <div className={`rounded-2xl bg-surface p-5 ${className}`}>
      <div className="text-xs text-muted">{label}</div>
      <div className="mt-2">{children}</div>
    </div>
  );
}

const DOT: Record<"ok" | "idle" | "busy" | "bad", string> = {
  ok: "bg-ok",
  idle: "bg-muted",
  busy: "bg-warn animate-pulse",
  bad: "bg-bad",
};

export function StatusDot({ tone }: { tone: keyof typeof DOT }) {
  return <span className={`inline-block size-2 rounded-full ${DOT[tone]}`} />;
}
