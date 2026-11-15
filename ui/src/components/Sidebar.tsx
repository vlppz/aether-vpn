import { motion } from "motion/react";
import { House, PanelLeftClose, PanelLeftOpen, Route, Rss, ScrollText, Server, Settings2, type LucideIcon } from "lucide-react";
import { cn } from "../lib/cn";
import { stripFlag } from "../lib/flags";
import { isTauri } from "../lib/env";
import { setUi, useApp, type Page, selectSelectedServer } from "../store";
import { MOD } from "../hooks/useGlobalShortcuts";
import { useT, type Key } from "../i18n";
import { StatusDot } from "./ui/StatusDot";
import { Tooltip } from "./ui/Tooltip";
import { Logo } from "./Logo";

const NAV: { page: Page; icon: LucideIcon; key: Key }[] = [
  { page: "home", icon: House, key: "nav.home" },
  { page: "servers", icon: Server, key: "nav.servers" },
  { page: "subscriptions", icon: Rss, key: "nav.subscriptions" },
  { page: "routing", icon: Route, key: "nav.routing" },
  { page: "logs", icon: ScrollText, key: "nav.logs" },
  { page: "settings", icon: Settings2, key: "nav.settings" },
];

export function Sidebar({ collapsed, onToggle }: { collapsed: boolean; onToggle: () => void }) {
  const t = useT();
  const page = useApp((s) => s.page);
  const status = useApp((s) => s.status);
  const platform = useApp((s) => s.platform);
  const server = useApp(selectSelectedServer);
  const mac = platform === "macos";

  return (
    <nav aria-label={t("nav.main")} className={cn("flex shrink-0 flex-col border-r border-line bg-surface transition-[width] duration-200 ease-out", collapsed ? "w-[56px]" : "w-[208px]")}>
      {/* macOS draws its traffic lights here */}
      {mac && <div data-tauri-drag-region className="h-10 shrink-0" />}
      <div data-tauri-drag-region className={cn("flex shrink-0 items-center gap-2 text-ink", mac ? "h-11" : "h-14", collapsed ? "justify-center" : "px-4")}>
        <Logo size={18} />
        {!collapsed && (
          <span data-tauri-drag-region className="font-serif text-[22px] leading-none tracking-[-0.01em]">
            Aether
          </span>
        )}
        {!collapsed && !isTauri && (
          <span className="mt-1 font-mono text-[10px] text-ink-3" title={t("titlebar.demoHint")}>
            {t("titlebar.demo")}
          </span>
        )}
      </div>

      <ul className={cn("mt-2 flex flex-col gap-px", collapsed ? "px-2" : "px-2.5")}>
        {NAV.map(({ page: p, icon: Icon, key }, i) => {
          const active = p === page;
          const item = (
            <button
              type="button"
              onClick={() => setUi({ page: p })}
              aria-current={active ? "page" : undefined}
              aria-label={collapsed ? t(key) : undefined}
              className={cn(
                "group relative flex h-8 w-full items-center gap-2.5 rounded-md text-[13.5px] transition-colors duration-100",
                collapsed ? "justify-center" : "px-2.5",
                active ? "bg-raised text-ink shadow-[inset_0_0_0_1px_var(--line)]" : "text-ink-2 hover:bg-hover hover:text-ink",
              )}
            >
              {active && (
                <motion.span
                  layoutId="nav-tick"
                  aria-hidden
                  className="absolute top-2 bottom-2 -left-[1px] w-[2px] rounded-full bg-accent"
                  transition={{ type: "tween", duration: 0.16, ease: "easeOut" }}
                />
              )}
              <Icon className={cn("size-4 shrink-0", active ? "text-ink" : "text-ink-3 group-hover:text-ink-2")} strokeWidth={1.5} aria-hidden />
              {!collapsed && <span className="flex-1 truncate text-left">{t(key)}</span>}
              {!collapsed && (
                <span className="font-mono text-[10.5px] text-ink-3 opacity-0 transition-opacity group-hover:opacity-100" aria-hidden>
                  {MOD}
                  {i + 1}
                </span>
              )}
            </button>
          );
          return (
            <li key={p}>
              {collapsed ? (
                <Tooltip content={t(key)} side="right" delay={150}>
                  {item}
                </Tooltip>
              ) : (
                item
              )}
            </li>
          );
        })}
      </ul>

      <div className="mt-auto border-t border-line">
        <div className={cn("flex h-11 items-center gap-2", collapsed ? "flex-col justify-center gap-0 py-2 h-auto" : "pr-2 pl-4")}>
          <button
            type="button"
            onClick={() => setUi({ page: "home" })}
            title={server ? stripFlag(server.name) : undefined}
            aria-label={t(`state.${status.state}`)}
            className={cn("flex min-w-0 items-center gap-2 text-left text-[12.5px] text-ink-2 hover:text-ink", collapsed ? "size-8 justify-center" : "flex-1")}
          >
            <StatusDot state={status.state} />
            {!collapsed && <span className="truncate">{t(`state.${status.state}`)}</span>}
          </button>
          <Tooltip content={collapsed ? t("nav.expand") : t("nav.collapse")} side={collapsed ? "right" : "top"}>
            <button
              type="button"
              onClick={onToggle}
              aria-label={collapsed ? t("nav.expand") : t("nav.collapse")}
              className="grid size-7 shrink-0 place-items-center rounded-md text-ink-3 hover:bg-hover hover:text-ink"
            >
              {collapsed ? <PanelLeftOpen className="size-4" strokeWidth={1.5} /> : <PanelLeftClose className="size-4" strokeWidth={1.5} />}
            </button>
          </Tooltip>
        </div>
      </div>
    </nav>
  );
}
