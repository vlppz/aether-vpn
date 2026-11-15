import type { ReactNode } from "react";
import { cn } from "../../lib/cn";

export function SettingRow({ label, description, children, className }: { label: ReactNode; description?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <div className={cn("flex min-h-[52px] items-center gap-6 border-t border-line py-2.5 first:border-t-0", className)}>
      <div className="min-w-0 flex-1">
        <div className="text-[13.5px] text-ink">{label}</div>
        {description && <div className="mt-0.5 text-[12px] text-ink-3">{description}</div>}
      </div>
      <div className="flex shrink-0 items-center gap-2">{children}</div>
    </div>
  );
}
