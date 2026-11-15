import { useId, type ReactNode } from "react";
import { motion } from "motion/react";
import { cn } from "../../lib/cn";

export interface SegmentOption<T extends string> {
  value: T;
  label: ReactNode;
  icon?: ReactNode;
  /** small coloured dot shown before the label (e.g. rule targets) */
  dot?: string;
  title?: string;
}

interface Props<T extends string> {
  value: T;
  options: SegmentOption<T>[];
  onChange: (value: T) => void;
  label: string;
  size?: "sm" | "md";
  disabled?: boolean;
  className?: string;
}

export function Segmented<T extends string>({ value, options, onChange, label, size = "md", disabled, className }: Props<T>) {
  const id = useId();
  const move = (dir: 1 | -1) => {
    const i = options.findIndex((o) => o.value === value);
    onChange(options[(i + dir + options.length) % options.length].value);
  };
  return (
    <div
      role="radiogroup"
      aria-label={label}
      aria-disabled={disabled}
      onKeyDown={(e) => {
        if (e.key === "ArrowRight" || e.key === "ArrowDown") {
          e.preventDefault();
          move(1);
        } else if (e.key === "ArrowLeft" || e.key === "ArrowUp") {
          e.preventDefault();
          move(-1);
        }
      }}
      className={cn("relative inline-flex items-center rounded-md border border-line bg-bg/60 p-0.5", disabled && "pointer-events-none opacity-50", className)}
    >
      {options.map((o) => {
        const active = o.value === value;
        return (
          <button
            key={o.value}
            type="button"
            role="radio"
            aria-checked={active}
            tabIndex={active ? 0 : -1}
            title={o.title}
            onClick={() => onChange(o.value)}
            className={cn(
              "relative z-0 inline-flex items-center justify-center gap-1.5 rounded-[5px] font-medium whitespace-nowrap transition-colors duration-150 [&_svg]:size-3.5",
              size === "sm" ? "h-6 px-2 text-[12px]" : "h-7 px-3 text-[12.5px]",
              active ? "text-ink" : "text-ink-3 hover:text-ink-2",
            )}
          >
            {active && (
              <motion.span
                layoutId={`seg-${id}`}
                aria-hidden
                className="absolute inset-0 -z-10 rounded-[5px] border border-line-strong bg-raised shadow-[0_1px_1px_rgb(0_0_0/0.04)]"
                transition={{ type: "tween", duration: 0.16, ease: "easeOut" }}
              />
            )}
            {o.dot && <span aria-hidden className="size-1.5 rounded-full" style={{ background: o.dot, opacity: active ? 1 : 0.55 }} />}
            {o.icon}
            {o.label}
          </button>
        );
      })}
    </div>
  );
}
