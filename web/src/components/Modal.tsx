import { type ReactNode, useEffect } from "react";

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
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && !locked && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, locked]);

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center overflow-y-auto overscroll-contain bg-onyx/70 p-4 backdrop-blur-sm"
      onClick={() => !locked && onClose()}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className="w-full max-w-md rounded-3xl bg-surface p-6"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold">{title}</h2>
          {!locked && (
            <button onClick={onClose} className="text-sm text-muted hover:text-alabaster">
              {closeLabel}
            </button>
          )}
        </div>
        <div className="mt-5">{children}</div>
      </div>
    </div>
  );
}
