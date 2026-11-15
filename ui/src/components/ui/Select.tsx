import { ChevronDown } from "lucide-react";
import { cn } from "../../lib/cn";

interface Props<T extends string> {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
  label: string;
  className?: string;
  size?: "sm" | "md";
}

/** Styled native <select> — keyboard and screen-reader friendly for free. */
export function Select<T extends string>({ value, options, onChange, label, className, size = "md" }: Props<T>) {
  return (
    <div className={cn("relative inline-flex", className)}>
      <select
        aria-label={label}
        value={value}
        onChange={(e) => onChange(e.target.value as T)}
        className={cn(
          "w-full appearance-none rounded-md border border-line-strong bg-raised pr-7 pl-2.5 text-ink outline-none transition-colors hover:border-ink-3/60 focus-visible:border-accent",
          size === "sm" ? "h-7 text-[12.5px]" : "h-8 text-[13px]",
        )}
      >
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      <ChevronDown className="pointer-events-none absolute top-1/2 right-2 size-3.5 -translate-y-1/2 text-ink-3" aria-hidden />
    </div>
  );
}
