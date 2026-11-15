import { useEffect, useId, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "motion/react";
import { X } from "lucide-react";
import { cn } from "../../lib/cn";
import { useT } from "../../i18n";
import { IconButton } from "./Button";

const FOCUSABLE = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** Focus the first field (or the panel), trap Tab inside, restore focus on close. */
function useFocusTrap(open: boolean, panel: React.RefObject<HTMLDivElement | null>, onClose: () => void) {
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  });
  useEffect(() => {
    if (!open) return;
    const prev = document.activeElement as HTMLElement | null;
    const raf = requestAnimationFrame(() => {
      const el = panel.current;
      if (!el) return;
      const first = el.querySelector<HTMLElement>("[data-autofocus]") ?? el.querySelector<HTMLElement>("input, textarea, select");
      (first ?? el).focus();
    });
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onCloseRef.current();
      } else if (e.key === "Tab" && panel.current) {
        const items = [...panel.current.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((x) => x.offsetParent !== null);
        if (items.length === 0) return;
        const first = items[0];
        const last = items[items.length - 1];
        if (e.shiftKey && document.activeElement === first) {
          e.preventDefault();
          last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("keydown", onKey, true);
      prev?.focus?.();
    };
  }, [open, panel]);
}

interface DialogProps {
  open: boolean;
  onClose: () => void;
  title?: ReactNode;
  description?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  className?: string;
  /** hide the default header */
  bare?: boolean;
}

export function Dialog({ open, onClose, title, description, children, footer, className, bare }: DialogProps) {
  const t = useT();
  const panel = useRef<HTMLDivElement>(null);
  const titleId = useId();
  useFocusTrap(open, panel, onClose);

  return createPortal(
    <AnimatePresence>
      {open && (
        <motion.div
          className="fixed inset-0 z-50 grid place-items-center p-6"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.14 }}
        >
          <div className="absolute inset-0 bg-[var(--scrim)]" onClick={onClose} aria-hidden />
          <motion.div
            ref={panel}
            role="dialog"
            aria-modal="true"
            aria-labelledby={title ? titleId : undefined}
            tabIndex={-1}
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 4 }}
            transition={{ duration: 0.16, ease: "easeOut" }}
            className={cn("float relative w-full max-w-[480px] outline-none", className)}
          >
            {!bare && (
              <div className="flex items-start gap-3 px-6 pt-5">
                <div className="min-w-0 flex-1">
                  {title && (
                    <h2 id={titleId} className="font-serif text-[24px] leading-tight">
                      {title}
                    </h2>
                  )}
                  {description && <p className="mt-1 text-[13px] text-ink-2">{description}</p>}
                </div>
                <IconButton label={t("common.close")} size="sm" onClick={onClose} className="-mr-2" tooltip={false}>
                  <X />
                </IconButton>
              </div>
            )}
            <div className={cn(!bare && "px-6 pt-4 pb-6")}>{children}</div>
            {footer && <div className="flex justify-end gap-2 border-t border-line px-6 py-3.5">{footer}</div>}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>,
    document.body,
  );
}

interface SheetProps {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  children: ReactNode;
  headerExtra?: ReactNode;
}

/** Right-side drawer. */
export function Sheet({ open, onClose, title, children, headerExtra }: SheetProps) {
  const t = useT();
  const panel = useRef<HTMLDivElement>(null);
  const titleId = useId();
  useFocusTrap(open, panel, onClose);

  return createPortal(
    <AnimatePresence>
      {open && (
        <motion.div className="fixed inset-0 z-50" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.16 }}>
          <div className="absolute inset-0 bg-[var(--scrim)]" onClick={onClose} aria-hidden />
          <motion.div
            ref={panel}
            role="dialog"
            aria-modal="true"
            aria-labelledby={titleId}
            tabIndex={-1}
            initial={{ x: 24, opacity: 0 }}
            animate={{ x: 0, opacity: 1 }}
            exit={{ x: 24, opacity: 0 }}
            transition={{ duration: 0.18, ease: "easeOut" }}
            className="absolute top-0 right-0 bottom-0 flex w-[min(420px,calc(100vw-24px))] flex-col border-l border-line-strong bg-raised shadow-[var(--shadow-float)] outline-none"
          >
            <div className="flex items-center gap-3 px-5 pt-5 pb-3">
              <h2 id={titleId} className="flex-1 font-serif text-[24px] leading-tight">
                {title}
              </h2>
              <IconButton label={t("common.close")} size="sm" onClick={onClose} tooltip={false}>
                <X />
              </IconButton>
            </div>
            {headerExtra && <div className="px-5 pb-3">{headerExtra}</div>}
            <div className="flex min-h-0 flex-1 flex-col">{children}</div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>,
    document.body,
  );
}
