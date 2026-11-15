import type { ReactNode } from "react";

export function EmptyState({ art, title, body, children }: { art?: ReactNode; title: ReactNode; body?: ReactNode; children?: ReactNode }) {
  return (
    <div className="mx-auto flex max-w-sm animate-[fade-in_.3s_ease-out] flex-col items-center px-6 py-10 text-center">
      {art && <div className="mb-6 text-ink-3">{art}</div>}
      <h3 className="font-serif text-[26px] leading-tight text-ink">{title}</h3>
      {body && <p className="mt-2 text-[13px] leading-relaxed text-ink-2">{body}</p>}
      {children && <div className="mt-6 flex flex-col items-center gap-3">{children}</div>}
    </div>
  );
}
