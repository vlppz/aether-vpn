import { useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

interface Props {
  content: ReactNode;
  children: ReactNode;
  side?: "top" | "bottom" | "right";
  delay?: number;
}

/**
 * Lightweight hover/focus tooltip rendered in a portal (never clipped by overflow).
 * Purely visual — triggers carry their own aria-label.
 */
export function Tooltip({ content, children, side = "bottom", delay = 500 }: Props) {
  const [pos, setPos] = useState<{ x: number; y: number } | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);

  useEffect(() => () => clearTimeout(timer.current), []);

  const show = (wrapper: HTMLElement) => {
    const el = (wrapper.firstElementChild as HTMLElement | null) ?? wrapper;
    clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      const r = el.getBoundingClientRect();
      if (side === "top") setPos({ x: r.left + r.width / 2, y: r.top - 6 });
      else if (side === "right") setPos({ x: r.right + 8, y: r.top + r.height / 2 });
      else setPos({ x: r.left + r.width / 2, y: r.bottom + 6 });
    }, delay);
  };
  const hide = () => {
    clearTimeout(timer.current);
    setPos(null);
  };

  const transform = side === "top" ? "translate(-50%, -100%)" : side === "right" ? "translate(0, -50%)" : "translate(-50%, 0)";

  return (
    <>
      <span
        className="contents"
        onMouseEnter={(e) => show(e.currentTarget)}
        onMouseLeave={hide}
        onFocus={(e) => {
          if ((e.target as HTMLElement).matches(":focus-visible")) show(e.currentTarget);
        }}
        onBlur={hide}
        onPointerDown={hide}
      >
        {children}
      </span>
      {pos &&
        createPortal(
          <div role="tooltip" className="pointer-events-none fixed z-[100] animate-[fade-in_.12s_ease-out]" style={{ left: pos.x, top: pos.y }}>
            <div style={{ transform }} className="rounded-[5px] bg-ink px-2 py-1 text-[11.5px] whitespace-nowrap text-bg">
              {content}
            </div>
          </div>,
          document.body,
        )}
    </>
  );
}
