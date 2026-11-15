import { memo, useRef, useState } from "react";
import { EllipsisVertical } from "lucide-react";
import type { ServerView } from "../lib/types";
import { cn } from "../lib/cn";
import { stripFlag } from "../lib/flags";
import { useT } from "../i18n";
import { Flag } from "./ui/Flag";
import { Latency } from "./ui/Latency";
import { ServerMeta } from "./ServerMeta";

interface Props {
  server: ServerView;
  selected: boolean;
  pending: boolean;
  compact?: boolean;
  renaming?: boolean;
  onSelect: (id: string) => void;
  onActivate: (id: string) => void;
  onMenu?: (id: string, at: { x: number; y: number }) => void;
  onRename?: (id: string, name: string | null) => void;
}

export const ServerRow = memo(function ServerRow({ server, selected, pending, compact, renaming, onSelect, onActivate, onMenu, onRename }: Props) {
  const t = useT();
  return (
    <div
      role="option"
      aria-selected={selected}
      tabIndex={0}
      onClick={() => onSelect(server.id)}
      onDoubleClick={() => onActivate(server.id)}
      onKeyDown={(e) => {
        if (e.target !== e.currentTarget) return;
        if (e.key === "Enter") onActivate(server.id);
        else if (e.key === " ") {
          e.preventDefault();
          onSelect(server.id);
        } else if ((e.key === "F10" && e.shiftKey) || e.key === "ContextMenu") {
          e.preventDefault();
          const r = e.currentTarget.getBoundingClientRect();
          onMenu?.(server.id, { x: r.right - 220, y: r.bottom - 6 });
        }
      }}
      onContextMenu={
        onMenu
          ? (e) => {
              e.preventDefault();
              onMenu(server.id, { x: e.clientX, y: e.clientY });
            }
          : undefined
      }
      className={cn(
        "group relative flex h-full cursor-pointer items-center gap-3 rounded-md px-3 transition-colors duration-100 focus-visible:outline-offset-[-1px]",
        selected ? "bg-raised shadow-[inset_0_0_0_1px_var(--line-strong)]" : "hover:bg-hover",
      )}
    >
      {selected && <span aria-hidden className="absolute top-2.5 bottom-2.5 left-0 w-[2px] rounded-full bg-accent" />}
      <Flag code={server.country} size={compact ? 17 : 18} />
      <div className="min-w-0 flex-1">
        {renaming && onRename ? (
          <RenameField initial={server.name} onDone={(name) => onRename(server.id, name)} label={t("servers.renameLabel")} />
        ) : (
          <div className={cn("truncate text-[13.5px]", selected ? "font-medium text-ink" : "text-ink")}>{stripFlag(server.name)}</div>
        )}
        <div className="mt-0.5 flex min-w-0 items-baseline gap-2">
          <ServerMeta server={server} />
          {!compact && server.address && (
            <span className="truncate font-mono text-[11px] text-ink-3/80">
              {server.address}
              {server.port ? `:${server.port}` : ""}
            </span>
          )}
        </div>
      </div>
      <Latency latency={server.latency} pending={pending} className="w-[72px]" />
      {onMenu && (
        <button
          type="button"
          aria-label={t("servers.actions")}
          onClick={(e) => {
            e.stopPropagation();
            const r = e.currentTarget.getBoundingClientRect();
            onMenu(server.id, { x: r.right - 216, y: r.bottom + 4 });
          }}
          onDoubleClick={(e) => e.stopPropagation()}
          className="grid size-7 shrink-0 place-items-center rounded-md text-ink-3 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100 hover:bg-hover hover:text-ink focus-visible:opacity-100"
        >
          <EllipsisVertical className="size-4" />
        </button>
      )}
    </div>
  );
});

function RenameField({ initial, onDone, label }: { initial: string; onDone: (name: string | null) => void; label: string }) {
  const [value, setValue] = useState(initial);
  const done = useRef(false);
  const finish = (name: string | null) => {
    if (done.current) return;
    done.current = true;
    onDone(name);
  };
  const result = () => (value.trim() && value.trim() !== initial ? value.trim() : null);
  return (
    <input
      autoFocus
      aria-label={label}
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onClick={(e) => e.stopPropagation()}
      onDoubleClick={(e) => e.stopPropagation()}
      onFocus={(e) => e.currentTarget.select()}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Enter") finish(result());
        if (e.key === "Escape") finish(null);
      }}
      onBlur={() => finish(result())}
      className="h-6 w-full rounded-[5px] border border-accent bg-raised px-1.5 text-[13.5px] text-ink outline-none"
    />
  );
}
