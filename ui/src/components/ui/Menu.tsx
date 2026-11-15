import { useEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "motion/react";
import { cn } from "../../lib/cn";

export type MenuEntry =
  | { label: string; icon?: ReactNode; onSelect: () => void; danger?: boolean; disabled?: boolean; hint?: ReactNode }
  | "separator";

interface Props {
  /** viewport coordinates to open at; null = closed */
  anchor: { x: number; y: number } | null;
  items: MenuEntry[];
  onClose: () => void;
  label: string;
}

const WIDTH = 216;
const ITEM_H = 32;

/** Context / kebab menu rendered in a portal with arrow-key navigation. */
export function Menu({ anchor, items, onClose, label }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  });

  useEffect(() => {
    if (!anchor) return;
    const raf = requestAnimationFrame(() => ref.current?.querySelector<HTMLElement>('[role="menuitem"]:not([disabled])')?.focus());
    const close = () => onCloseRef.current();
    window.addEventListener("resize", close);
    window.addEventListener("blur", close);
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", close);
      window.removeEventListener("blur", close);
    };
  }, [anchor]);

  let left = 0;
  let top = 0;
  let origin = "top left";
  if (anchor) {
    const height = items.reduce((h, i) => h + (i === "separator" ? 9 : ITEM_H), 10);
    const flipX = anchor.x + WIDTH > window.innerWidth - 8;
    const flipY = anchor.y + height > window.innerHeight - 8;
    left = flipX ? Math.max(8, anchor.x - WIDTH) : anchor.x;
    top = flipY ? Math.max(8, anchor.y - height) : anchor.y;
    origin = `${flipY ? "bottom" : "top"} ${flipX ? "right" : "left"}`;
  }

  const onKeyDown = (e: React.KeyboardEvent) => {
    const els = [...(ref.current?.querySelectorAll<HTMLElement>('[role="menuitem"]:not([disabled])') ?? [])];
    const i = els.indexOf(document.activeElement as HTMLElement);
    if (e.key === "ArrowDown") {
      e.preventDefault();
      els[(i + 1) % els.length]?.focus();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      els[(i - 1 + els.length) % els.length]?.focus();
    } else if (e.key === "Escape" || e.key === "Tab") {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    }
  };

  return createPortal(
    <AnimatePresence>
      {anchor && (
        <div className="fixed inset-0 z-[60]" onPointerDown={onClose} onContextMenu={(e) => (e.preventDefault(), onClose())}>
          <motion.div
            ref={ref}
            role="menu"
            aria-label={label}
            onPointerDown={(e) => e.stopPropagation()}
            onKeyDown={onKeyDown}
            initial={{ opacity: 0, y: -2 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, transition: { duration: 0.08 } }}
            transition={{ duration: 0.12, ease: "easeOut" }}
            style={{ left, top, width: WIDTH, transformOrigin: origin }}
            className="float absolute p-1"
          >
            {items.map((item, i) =>
              item === "separator" ? (
                <div key={`sep-${i}`} className="mx-1 my-1 h-px bg-line" role="separator" />
              ) : (
                <button
                  key={item.label}
                  type="button"
                  role="menuitem"
                  disabled={item.disabled}
                  onClick={() => {
                    onClose();
                    item.onSelect();
                  }}
                  className={cn(
                    "flex h-8 w-full items-center gap-2.5 rounded-[5px] px-2 text-left text-[13px] outline-none transition-colors disabled:opacity-40",
                    "[&>svg]:size-3.5 [&>svg]:shrink-0",
                    item.danger
                      ? "text-danger hover:bg-danger/10 focus-visible:bg-danger/10"
                      : "text-ink hover:bg-hover focus-visible:bg-hover [&>svg]:text-ink-3",
                  )}
                  style={{ outline: "none" }}
                >
                  {item.icon}
                  <span className="flex-1 truncate">{item.label}</span>
                  {item.hint}
                </button>
              ),
            )}
          </motion.div>
        </div>
      )}
    </AnimatePresence>,
    document.body,
  );
}
