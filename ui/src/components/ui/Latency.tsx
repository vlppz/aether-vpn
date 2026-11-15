import { cn } from "../../lib/cn";
import { latencyTone, type LatencyTone } from "../../lib/format";
import { useT } from "../../i18n";

const dot: Record<LatencyTone, string> = {
  good: "bg-ok",
  warn: "bg-warn",
  bad: "bg-danger",
  fail: "bg-danger",
  none: "bg-ink-3/50",
};

/** Mono latency figure with a 6px status dot. */
export function Latency({ latency, pending, className }: { latency: number | null; pending?: boolean; className?: string }) {
  const t = useT();
  const tone = latencyTone(latency);
  const text = pending ? "···" : latency === null ? "—" : latency < 0 ? t("servers.timeout") : t("common.ms", { n: latency });
  return (
    <span
      className={cn("tabular inline-flex shrink-0 items-center justify-end gap-1.5 font-mono text-[12px]", tone === "fail" ? "text-danger" : tone === "none" ? "text-ink-3" : "text-ink-2", className)}
      aria-busy={pending || undefined}
    >
      <span aria-hidden className={cn("size-1.5 rounded-full", pending ? "animate-[breathe_1s_ease-in-out_infinite] bg-ink-3" : dot[tone])} />
      {text}
    </span>
  );
}
