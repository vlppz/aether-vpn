import { cn } from "../../lib/cn";

interface Props {
  checked: boolean;
  onChange: (value: boolean) => void;
  label: string;
  disabled?: boolean;
  size?: "sm" | "md";
}

export function Switch({ checked, onChange, label, disabled, size = "md" }: Props) {
  const sm = size === "sm";
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        "relative inline-flex shrink-0 items-center rounded-full transition-colors duration-150 disabled:opacity-40",
        sm ? "h-4 w-7" : "h-[18px] w-8",
        checked ? "bg-accent" : "bg-ink/18 dark:bg-ink/22",
      )}
    >
      <span
        aria-hidden
        className={cn(
          "absolute left-[2px] rounded-full shadow-[0_1px_2px_rgb(0_0_0/0.25)] transition-transform duration-150 ease-out",
          checked ? "bg-white dark:bg-bg" : "bg-white dark:bg-ink-2",
          sm ? "size-3" : "size-[14px]",
          checked && (sm ? "translate-x-3" : "translate-x-[14px]"),
        )}
      />
    </button>
  );
}
