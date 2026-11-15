import type { ReactNode } from "react";

export function PageHeader({ title, subtitle, children }: { title: ReactNode; subtitle?: ReactNode; children?: ReactNode }) {
  return (
    <header className="flex shrink-0 flex-wrap items-end gap-x-4 gap-y-3 pt-2 pb-5">
      <div className="min-w-0 flex-1">
        <h1 className="font-serif text-[34px] leading-none tracking-[-0.01em] text-ink">{title}</h1>
        {subtitle && <p className="mt-2 text-[12.5px] text-ink-3">{subtitle}</p>}
      </div>
      {children && <div className="flex flex-wrap items-center gap-2">{children}</div>}
    </header>
  );
}
