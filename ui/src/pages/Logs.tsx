import { memo, useCallback, useDeferredValue, useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { AnimatePresence, motion } from "motion/react";
import { ArrowDownToLine, Copy, Eraser, ScrollText, Search } from "lucide-react";
import { toast } from "sonner";
import type { LogLevel, LogLine } from "../lib/types";
import { cn } from "../lib/cn";
import { formatClock } from "../lib/format";
import { outboundTone, parseAccess } from "../lib/logparse";
import { copyText } from "../lib/api";
import { useApp } from "../store";
import { attempt, clearLogs } from "../store/actions";
import { useT } from "../i18n";
import { PageHeader } from "./PageHeader";
import { Panel } from "../components/ui/Panel";
import { Input } from "../components/ui/Input";
import { Button } from "../components/ui/Button";
import { Switch } from "../components/ui/Switch";
import { EmptyState } from "../components/ui/EmptyState";

const LEVELS: LogLevel[] = ["access", "info", "warning", "error", "debug", "app"];

const LEVEL_STYLE: Record<LogLevel, { dot: string; text: string }> = {
  debug: { dot: "bg-ink-3/60", text: "text-ink-3" },
  info: { dot: "bg-ink-2", text: "text-ink-2" },
  warning: { dot: "bg-warn", text: "text-warn" },
  error: { dot: "bg-danger", text: "text-danger" },
  access: { dot: "bg-ink-3", text: "text-ink-3" },
  app: { dot: "bg-accent", text: "text-accent" },
};

const LEVEL_TAG: Record<LogLevel, string> = { debug: "DBG", info: "INF", warning: "WRN", error: "ERR", access: "ACC", app: "APP" };

export function Logs() {
  const t = useT();
  const logs = useApp((s) => s.logs);
  const [enabled, setEnabled] = useState<Record<LogLevel, boolean>>({ debug: true, info: true, warning: true, error: true, access: true, app: true });
  const [query, setQuery] = useState("");
  const [follow, setFollow] = useState(true);
  const deferredQuery = useDeferredValue(query.trim().toLowerCase());
  const parentRef = useRef<HTMLDivElement>(null);

  const counts = useMemo(() => {
    const c: Record<LogLevel, number> = { debug: 0, info: 0, warning: 0, error: 0, access: 0, app: 0 };
    for (const l of logs) c[l.level]++;
    return c;
  }, [logs]);

  const visible = useMemo(
    () => logs.filter((l) => enabled[l.level] && (!deferredQuery || l.message.toLowerCase().includes(deferredQuery))),
    [logs, enabled, deferredQuery],
  );

  // eslint-disable-next-line react-hooks/incompatible-library -- virtualizer returns stable functions; fine here
  const virtualizer = useVirtualizer({
    count: visible.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 24,
    overscan: 20,
  });

  const last = visible[visible.length - 1];
  useEffect(() => {
    if (!follow || !last) return;
    const raf = requestAnimationFrame(() => {
      const el = parentRef.current;
      if (el) el.scrollTop = el.scrollHeight;
    });
    return () => cancelAnimationFrame(raf);
  }, [follow, last]);

  // pause following only when the user scrolls *up*; resume when they reach the bottom
  const lastTop = useRef(0);
  const onScroll = useCallback(() => {
    const el = parentRef.current;
    if (!el) return;
    const fromBottom = el.scrollHeight - el.scrollTop - el.clientHeight;
    const movedUp = el.scrollTop < lastTop.current - 2;
    lastTop.current = el.scrollTop;
    if (fromBottom < 24) setFollow(true);
    else if (movedUp) setFollow(false);
  }, []);

  const copyAll = async () => {
    const text = visible.map((l) => `${formatClock(l.ts)} [${l.level}] ${l.message}`).join("\n");
    const ok = await attempt(() => copyText(text).then(() => true));
    if (ok) toast.success(t("toast.logsCopied", { n: visible.length }));
  };

  return (
    <div className="flex h-full flex-col">
      <PageHeader title={t("logs.title")} subtitle={t("logs.count", { shown: visible.length.toLocaleString(), total: logs.length.toLocaleString() })}>
        <Button variant="secondary" icon={<Copy />} onClick={copyAll} disabled={!visible.length}>
          {t("logs.copy")}
        </Button>
        <Button variant="secondary" icon={<Eraser />} onClick={() => void clearLogs()} disabled={!logs.length}>
          {t("logs.clear")}
        </Button>
      </PageHeader>

      <Panel className="relative flex min-h-0 flex-1 flex-col overflow-hidden">
        <div className="flex flex-wrap items-center gap-2 border-b border-line px-3 py-2.5">
          <Input
            icon={<Search />}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Escape" && setQuery("")}
            placeholder={t("logs.search")}
            aria-label={t("logs.search")}
            className="w-[220px]"
            mono
          />
          <div className="flex flex-wrap items-center gap-1" role="group" aria-label="Levels">
            {LEVELS.map((lv) => (
              <button
                key={lv}
                type="button"
                aria-pressed={enabled[lv]}
                onClick={() => setEnabled((e) => ({ ...e, [lv]: !e[lv] }))}
                className={cn(
                  "flex h-7 items-center gap-1.5 rounded-md border px-2 text-[12px] transition-colors",
                  enabled[lv] ? "border-line-strong bg-raised text-ink" : "border-transparent text-ink-3 hover:text-ink-2",
                )}
              >
                <span className={cn("size-1.5 rounded-full", LEVEL_STYLE[lv].dot, !enabled[lv] && "opacity-40")} />
                {t(`level.${lv}`)}
                <span className="tabular font-mono text-[10.5px] text-ink-3">{counts[lv]}</span>
              </button>
            ))}
          </div>
          <label className="ml-auto flex items-center gap-2 text-[12.5px] text-ink-2">
            {t("logs.follow")}
            <Switch size="sm" checked={follow} onChange={setFollow} label={t("logs.follow")} />
          </label>
        </div>

        {visible.length === 0 ? (
          <div className="grid flex-1 place-items-center">
            <EmptyState
              art={<ScrollText className="size-6" strokeWidth={1.25} />}
              title={t("logs.empty")}
              body={t("logs.emptyBody")}
            />
          </div>
        ) : (
          <div ref={parentRef} onScroll={onScroll} data-selectable className="min-h-0 flex-1 overflow-auto py-2 font-mono text-[11.5px] leading-[20px]">
            <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
              {virtualizer.getVirtualItems().map((v) => (
                <div key={v.key} data-index={v.index} ref={virtualizer.measureElement} className="absolute inset-x-0 top-0" style={{ transform: `translateY(${v.start}px)` }}>
                  <LogRow line={visible[v.index]} query={deferredQuery} />
                </div>
              ))}
            </div>
          </div>
        )}

        <AnimatePresence>
          {!follow && visible.length > 0 && (
            <motion.div initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: 10 }} className="absolute right-5 bottom-4">
              <Button variant="primary" size="sm" icon={<ArrowDownToLine />} onClick={() => setFollow(true)}>
                {t("logs.jump")}
              </Button>
            </motion.div>
          )}
        </AnimatePresence>
      </Panel>
    </div>
  );
}

function Highlight({ text, query }: { text: string; query: string }) {
  if (!query) return <>{text}</>;
  const lower = text.toLowerCase();
  const parts: React.ReactNode[] = [];
  let i = 0;
  let k = 0;
  for (;;) {
    const j = lower.indexOf(query, i);
    if (j < 0) break;
    if (j > i) parts.push(text.slice(i, j));
    parts.push(
      <mark key={k++} className="rounded-[2px] bg-warn/30 px-px text-ink">
        {text.slice(j, j + query.length)}
      </mark>,
    );
    i = j + query.length;
  }
  parts.push(text.slice(i));
  return <>{parts}</>;
}

const OUT_STYLE = {
  proxy: "text-accent",
  direct: "text-ok",
  block: "text-danger",
  other: "text-ink-2",
};

const LogRow = memo(function LogRow({ line, query }: { line: LogLine; query: string }) {
  const style = LEVEL_STYLE[line.level];
  const access = line.level === "access" ? parseAccess(line.message) : null;
  return (
    <div className="flex gap-4 px-4 py-px hover:bg-hover">
      <span className="shrink-0 text-ink-3">{formatClock(line.ts)}</span>
      <span className={cn("w-7 shrink-0", style.text)}>{LEVEL_TAG[line.level]}</span>
      {access ? (
        <span className="flex min-w-0 flex-1 flex-wrap items-center gap-x-2">
          {access.network && <span className="w-6 text-[10.5px] text-ink-3 uppercase">{access.network}</span>}
          <span className={cn("break-all", access.accepted ? "text-ink" : "text-danger line-through")}>
            <Highlight text={access.dest} query={query} />
          </span>
          <span className="text-ink-3" aria-hidden>
            →
          </span>
          {access.outbound && (
            <span className={OUT_STYLE[outboundTone(access.outbound)]}>
              <Highlight text={access.outbound} query={query} />
            </span>
          )}
          <span className="text-ink-3">
            {access.inbound} · <Highlight text={access.from} query={query} />
            {access.rest && ` · ${access.rest}`}
          </span>
        </span>
      ) : (
        <span className={cn("min-w-0 flex-1 break-words whitespace-pre-wrap", line.level === "error" ? "text-danger" : "text-ink")}>
          <Highlight text={line.message} query={query} />
        </span>
      )}
    </div>
  );
});
