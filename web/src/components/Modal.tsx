import { type ReactNode, useEffect } from "react";

/** A centred dialog over a dimmed page. Escape or a click outside closes it, unless `locked`. */
export function Modal({
  title,
  onClose,
  locked = false,
  children,
}: {
  title: string;
  onClose: () => void;
  locked?: boolean;
  children: ReactNode;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && !locked && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, locked]);

  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-onyx/70 p-4 backdrop-blur-sm" onClick={() => !locked && onClose()}>
      <div className="w-full max-w-md rounded-3xl bg-surface p-6" onClick={(e) => e.stopPropagation()}>
        <div className="flex items-center justify-between">
          <h2 className="text-lg font-semibold">{title}</h2>
          {!locked && (
            <button onClick={onClose} className="text-sm text-muted hover:text-alabaster">
              Close
            </button>
          )}
        </div>
        <div className="mt-5">{children}</div>
      </div>
    </div>
  );
}
