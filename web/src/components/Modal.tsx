import { type ReactNode, useCallback, useEffect, useState } from "react";

/** Stops the page behind a dialog from scrolling, without the layout jumping when the scrollbar goes. */
function useScrollLock() {
  useEffect(() => {
    const { body, documentElement } = document;
    const gap = window.innerWidth - documentElement.clientWidth;
    const previous = { overflow: body.style.overflow, paddingRight: body.style.paddingRight };
    body.style.overflow = "hidden";
    if (gap > 0) body.style.paddingRight = `${gap}px`;
    return () => {
      body.style.overflow = previous.overflow;
      body.style.paddingRight = previous.paddingRight;
    };
  }, []);
}

/** How long the closing animation runs before the dialog unmounts. */
const CLOSE_MS = 150;

/** A centred dialog over a dimmed page. Escape or a click outside closes it, unless `locked`. */
export function Modal({
  title,
  onClose,
  locked = false,
  closeLabel = "Close",
  children,
}: {
  title: string;
  onClose: () => void;
  locked?: boolean;
  closeLabel?: string;
  children: ReactNode;
}) {
  useScrollLock();
  const [leaving, setLeaving] = useState(false);
  // Play the exit, then let the parent unmount. A second close while leaving is ignored.
  const close = useCallback(() => {
    if (locked || leaving) return;
    setLeaving(true);
    setTimeout(onClose, CLOSE_MS);
  }, [locked, leaving, onClose]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && close();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [close]);

  return (
    <div
      className={`fixed inset-0 z-50 grid place-items-center overflow-y-auto overscroll-contain bg-onyx/70 p-4 backdrop-blur-sm ${
        leaving ? "animate-fade-out" : "animate-fade"
      }`}
      onClick={close}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className={`w-full max-w-md rounded-3xl bg-surface p-6 shadow-2xl shadow-onyx/60 ${leaving ? "animate-pop-out" : "animate-pop"}`}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold">{title}</h2>
          {!locked && (
            <button onClick={close} className="-mr-2 h-8 rounded-full px-2 text-sm text-muted transition-colors hover:text-alabaster">
              {closeLabel}
            </button>
          )}
        </div>
        <div className="mt-5">{children}</div>
      </div>
    </div>
  );
}
