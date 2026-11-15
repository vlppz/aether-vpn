import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { ChevronRight, Copy, Pencil, Plug, QrCode, RefreshCw, SearchX, Trash, Zap } from "lucide-react";
import { cn } from "../lib/cn";
import { countryName } from "../lib/flags";
import { useApp } from "../store";
import { filterServers, groupServers, MANUAL_GROUP, sortServers, type ServerSort } from "../store/reducers";
import { copyServerLink, pingServer, refreshSubscription, removeServer, renameServer, selectServer } from "../store/actions";
import { useLang, useT } from "../i18n";
import { Menu, type MenuEntry } from "./ui/Menu";
import { ServerRow } from "./ServerRow";
import { QrDialog } from "./QrDialog";
import { EmptyState } from "./ui/EmptyState";

type Item =
  | { type: "group"; key: string; id: string; name: string; count: number; isSub: boolean; collapsed: boolean }
  | { type: "server"; key: string; id: string; index: number };

interface Props {
  query: string;
  sort: ServerSort;
  compact?: boolean;
  /** called after a server was picked with a single click (used by the picker sheet) */
  onPicked?: () => void;
}

export function ServerList({ query, sort, compact, onPicked }: Props) {
  const t = useT();
  const lang = useLang();
  const servers = useApp((s) => s.servers);
  const subscriptions = useApp((s) => s.subscriptions);
  const selectedId = useApp((s) => s.selectedId);
  const pinging = useApp((s) => s.pinging);
  const refreshing = useApp((s) => s.refreshing);

  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  const [menu, setMenu] = useState<{ id: string; at: { x: number; y: number } } | null>(null);
  const [qrId, setQrId] = useState<string | null>(null);
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [intro, setIntro] = useState(true);
  const parentRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const id = setTimeout(() => setIntro(false), 900);
    return () => clearTimeout(id);
  }, []);

  const byId = useMemo(() => new Map(servers.map((s) => [s.id, s])), [servers]);

  const items = useMemo<Item[]>(() => {
    const filtered = filterServers(servers, query, (c) => countryName(c, lang));
    const groups = groupServers(sortServers(filtered, sort), subscriptions, t("servers.manual"));
    const searching = query.trim().length > 0;
    const out: Item[] = [];
    let index = 0;
    for (const g of groups) {
      if (searching && g.servers.length === 0) continue;
      const isCollapsed = !searching && !!collapsed[g.id];
      out.push({ type: "group", key: `g-${g.id}`, id: g.id, name: g.name, count: g.servers.length, isSub: g.id !== MANUAL_GROUP, collapsed: isCollapsed });
      if (!isCollapsed) for (const s of g.servers) out.push({ type: "server", key: s.id, id: s.id, index: index++ });
    }
    return out;
  }, [servers, subscriptions, query, sort, collapsed, lang, t]);

  const rowH = compact ? 52 : 56;
  // eslint-disable-next-line react-hooks/incompatible-library -- virtualizer returns stable functions; fine here
  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: (i) => (items[i].type === "group" ? 40 : rowH),
    getItemKey: (i) => items[i].key,
    overscan: 8,
  });

  const onSelect = useCallback(
    (id: string) => {
      void selectServer(id);
      onPicked?.();
    },
    [onPicked],
  );
  const onActivate = useCallback(
    (id: string) => {
      void selectServer(id, true);
      onPicked?.();
    },
    [onPicked],
  );
  const onMenu = useCallback((id: string, at: { x: number; y: number }) => setMenu({ id, at }), []);
  const onRename = useCallback((id: string, name: string | null) => {
    setRenamingId(null);
    if (name) void renameServer(id, name);
  }, []);

  const menuItems: MenuEntry[] = menu
    ? [
        { label: t("servers.connect"), icon: <Plug />, onSelect: () => void selectServer(menu.id, true) },
        { label: t("servers.ping"), icon: <Zap />, onSelect: () => void pingServer(menu.id) },
        "separator",
        { label: t("servers.copyLink"), icon: <Copy />, onSelect: () => void copyServerLink(menu.id) },
        { label: t("servers.showQr"), icon: <QrCode />, onSelect: () => setQrId(menu.id) },
        { label: t("servers.rename"), icon: <Pencil />, onSelect: () => setRenamingId(menu.id) },
        "separator",
        { label: t("servers.delete"), icon: <Trash />, danger: true, onSelect: () => void removeServer(menu.id) },
      ]
    : [];

  if (items.length === 0 && query.trim()) {
    return (
      <EmptyState
        art={<SearchX className="size-6" strokeWidth={1.25} />}
        title={t("servers.noResults", { q: query.trim() })}
      />
    );
  }

  return (
    <>
      <div ref={parentRef} role="listbox" aria-label={t("servers.title")} className={cn("min-h-0 flex-1 overflow-y-auto", compact ? "px-2 pb-3" : "px-2 pb-3")}>
        <div className="relative w-full" style={{ height: virtualizer.getTotalSize() }}>
          {virtualizer.getVirtualItems().map((v) => {
            const item = items[v.index];
            return (
              <div
                key={v.key}
                className="absolute inset-x-0 top-0"
                style={{ height: v.size, transform: `translateY(${v.start}px)` }}
              >
                <div className={cn("h-full", intro && "animate-[fade-in_.25s_ease-out_both]")} style={intro ? { animationDelay: `${Math.min(v.index, 14) * 18}ms` } : undefined}>
                {item.type === "group" ? (
                  <GroupHeader
                    item={item}
                    compact={compact}
                    refreshing={!!refreshing[item.id]}
                    onToggle={() => setCollapsed((c) => ({ ...c, [item.id]: !c[item.id] }))}
                    refreshLabel={t("servers.refreshGroup")}
                    toggleLabel={t("servers.toggleGroup")}
                  />
                ) : (
                  <div className="h-full pb-0.5">
                    <ServerRow
                      server={byId.get(item.id)!}
                      selected={item.id === selectedId}
                      pending={!!pinging[item.id]}
                      compact={compact}
                      renaming={renamingId === item.id}
                      onSelect={onSelect}
                      onActivate={onActivate}
                      onMenu={compact ? undefined : onMenu}
                      onRename={onRename}
                    />
                  </div>
                )}
                </div>
              </div>
            );
          })}
        </div>
      </div>
      <Menu anchor={menu?.at ?? null} items={menuItems} onClose={() => setMenu(null)} label={t("servers.actions")} />
      <QrDialog serverId={qrId} onClose={() => setQrId(null)} />
    </>
  );
}

function GroupHeader({
  item,
  compact,
  refreshing,
  onToggle,
  refreshLabel,
  toggleLabel,
}: {
  item: Extract<Item, { type: "group" }>;
  compact?: boolean;
  refreshing: boolean;
  onToggle: () => void;
  refreshLabel: string;
  toggleLabel: string;
}) {
  return (
    <div className="flex h-full items-end pb-1">
      <div className="flex w-full items-center gap-1">
        <button
          type="button"
          onClick={onToggle}
          aria-expanded={!item.collapsed}
          aria-label={`${item.name} — ${toggleLabel}`}
          className="flex h-7 min-w-0 flex-1 items-center gap-2 rounded-md px-2 text-left transition-colors hover:text-ink [&:hover_.label]:text-ink-2"
        >
          <ChevronRight className={cn("size-3 shrink-0 text-ink-3 transition-transform duration-150", !item.collapsed && "rotate-90")} aria-hidden />
          <span className="label truncate">{item.name}</span>
          <span className="tabular font-mono text-[11px] text-ink-3">{item.count}</span>
          <span className="ml-2 h-px flex-1 bg-line" aria-hidden />
        </button>
        {item.isSub && !compact && (
          <button
            type="button"
            aria-label={refreshLabel}
            title={refreshLabel}
            onClick={() => void refreshSubscription(item.id)}
            className="grid size-7 place-items-center rounded-md text-ink-3 transition-colors hover:bg-hover hover:text-ink"
          >
            <RefreshCw className={cn("size-3.5", refreshing && "animate-spin")} strokeWidth={1.5} />
          </button>
        )}
      </div>
    </div>
  );
}
