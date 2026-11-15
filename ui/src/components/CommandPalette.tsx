import { useEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "motion/react";
import { Command } from "cmdk";
import {
  Activity,
  ClipboardPaste,
  Download,
  Globe,
  House,
  Network,
  Power,
  PowerOff,
  RefreshCw,
  Route,
  Rss,
  ScrollText,
  Search,
  Server,
  Settings2,
  SunMoon,
  Timer,
  X,
  Zap,
  type LucideIcon,
} from "lucide-react";
import { toast } from "sonner";
import { api } from "../lib/api";
import { countryName, stripFlag } from "../lib/flags";
import { setUi, useApp, PAGES, type Page } from "../store";
import {
  attempt,
  importClipboard,
  pingAll,
  refreshAllSubscriptions,
  selectFastest,
  selectServer,
  setMode,
  setPreset,
  testRealDelay,
  toggleConnection,
  toggleTheme,
} from "../store/actions";
import { MOD } from "../hooks/useGlobalShortcuts";
import { useLang, useT } from "../i18n";
import { Flag } from "./ui/Flag";
import { Kbd } from "./ui/Kbd";
import { Latency } from "./ui/Latency";
import { ServerMeta } from "./ServerMeta";

const PAGE_ICON: Record<Page, LucideIcon> = {
  home: House,
  servers: Server,
  subscriptions: Rss,
  routing: Route,
  logs: ScrollText,
  settings: Settings2,
};

export function CommandPalette() {
  const t = useT();
  const open = useApp((s) => s.paletteOpen);
  const close = () => setUi({ paletteOpen: false });

  return createPortal(
    <AnimatePresence>
      {open && (
        <motion.div
          className="fixed inset-0 z-[70] flex justify-center px-6 pt-[12vh]"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.12 }}
        >
          <div className="absolute inset-0 bg-[var(--scrim)]" onClick={close} aria-hidden />
          <motion.div
            initial={{ opacity: 0, y: -4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.14, ease: "easeOut" }}
            className="relative h-fit w-full max-w-[560px]"
            role="dialog"
            aria-modal="true"
            aria-label={t("palette.label")}
          >
            <Palette onClose={close} />
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>,
    document.body,
  );
}

function Palette({ onClose }: { onClose: () => void }) {
  const t = useT();
  const lang = useLang();
  const status = useApp((s) => s.status);
  const servers = useApp((s) => s.servers);
  const selectedId = useApp((s) => s.selectedId);
  const mode = useApp((s) => s.settings.core.mode);
  const preset = useApp((s) => s.settings.core.routing.preset);
  const inputRef = useRef<HTMLInputElement>(null);
  const connected = status.state === "connected" || status.state === "connecting";

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);

  const run = (fn: () => unknown) => () => {
    onClose();
    void fn();
  };

  return (
    <Command
      label={t("palette.label")}
      loop
      className="float overflow-hidden"
      filter={(value, search, keywords) => {
        const hay = `${value} ${(keywords ?? []).join(" ")}`.toLowerCase();
        return search
          .toLowerCase()
          .split(/\s+/)
          .filter(Boolean)
          .every((tok) => hay.includes(tok))
          ? 1
          : 0;
      }}
    >
      <div className="flex items-center gap-2.5 border-b border-line px-3.5">
        <Search className="size-4 shrink-0 text-ink-3" strokeWidth={1.5} aria-hidden />
        <Command.Input
          ref={inputRef}
          autoFocus
          placeholder={t("palette.placeholder")}
          className="h-12 flex-1 bg-transparent text-[14.5px] text-ink outline-none placeholder:text-ink-3 focus-visible:outline-none"
        />
        <button type="button" onClick={onClose} aria-label={t("common.close")} className="grid size-7 place-items-center rounded-md text-ink-3 hover:bg-hover hover:text-ink">
          <X className="size-4" />
        </button>
      </div>
      <Command.List className="max-h-[min(420px,60vh)] scroll-py-1.5 overflow-y-auto p-1.5">
        <Command.Empty className="py-10 text-center text-[13px] text-ink-3">{t("palette.empty")}</Command.Empty>

        <Group heading={t("palette.connection")}>
          <Item icon={connected ? PowerOff : Power} onSelect={run(toggleConnection)} hint={<Kbd>Space</Kbd>} keywords={["vpn", "toggle", "on", "off"]}>
            {connected ? t("palette.disconnect") : t("palette.connect")}
          </Item>
          {status.state === "connected" && (
            <Item
              icon={Timer}
              onSelect={run(async () => {
                const ms = await testRealDelay();
                if (ms !== undefined) toast.success(t("toast.realDelay", { ms }));
              })}
            >
              {t("palette.testDelay")}
            </Item>
          )}
          <Item icon={Activity} onSelect={run(pingAll)} keywords={["latency", "test"]}>
            {t("palette.pingAll")}
          </Item>
          <Item icon={Zap} onSelect={run(selectFastest)} keywords={["best", "auto"]}>
            {t("palette.fastest")}
          </Item>
        </Group>

        <Group heading={t("palette.mode")}>
          <Item icon={Globe} onSelect={run(() => setMode("proxy"))} active={mode === "proxy"} keywords={["proxy", "mode"]}>
            {t("palette.useMode", { mode: t("mode.proxy") })}
          </Item>
          <Item icon={Network} onSelect={run(() => setMode("tun"))} active={mode === "tun"} keywords={["tun", "mode", "vpn"]}>
            {t("palette.useMode", { mode: t("mode.tun") })}
          </Item>
        </Group>

        <Group heading={t("palette.routing")}>
          {(["global", "smart", "direct"] as const).map((p) => (
            <Item key={p} icon={Route} onSelect={run(() => setPreset(p))} active={preset === p} keywords={["routing", "preset", p]}>
              {t("palette.usePreset", { preset: t(`preset.${p}`) })}
            </Item>
          ))}
        </Group>

        <Group heading={t("palette.navigate")}>
          {PAGES.map((p, i) => (
            <Item key={p} icon={PAGE_ICON[p]} onSelect={run(() => setUi({ page: p }))} hint={<Kbd>{`${MOD}${i + 1}`}</Kbd>} keywords={["go", "page", p]}>
              {t(`nav.${p}`)}
            </Item>
          ))}
        </Group>

        <Group heading={t("palette.general")}>
          <Item icon={ClipboardPaste} onSelect={run(importClipboard)} hint={<Kbd>{`${MOD}V`}</Kbd>} keywords={["paste", "import", "link"]}>
            {t("palette.importClipboard")}
          </Item>
          <Item icon={Download} onSelect={run(() => setUi({ importOpen: true }))} keywords={["import", "add"]}>
            {t("palette.importDialog")}
          </Item>
          <Item icon={RefreshCw} onSelect={run(refreshAllSubscriptions)} keywords={["update", "subscription"]}>
            {t("palette.refreshSubs")}
          </Item>
          <Item icon={SunMoon} onSelect={run(toggleTheme)} keywords={["dark", "light", "theme"]}>
            {t("palette.toggleTheme")}
          </Item>
          <Item icon={X} onSelect={run(() => attempt(() => api.quitApp()))} keywords={["exit", "quit"]}>
            {t("palette.quit")}
          </Item>
        </Group>

        {servers.length > 0 && (
          <Group heading={t("palette.servers")}>
            {servers.map((s) => (
              <Command.Item
                key={s.id}
                value={`server ${s.id}`}
                keywords={[s.name, s.protocol, s.transport, s.security, s.country ?? "", countryName(s.country, lang)]}
                onSelect={run(() => selectServer(s.id))}
                className="flex h-9 cursor-pointer items-center gap-3 rounded-[5px] px-2.5 text-[13px] text-ink data-[selected=true]:bg-hover"
              >
                <Flag code={s.country} size={15} />
                <span className="min-w-0 flex-1 truncate">{stripFlag(s.name)}</span>
                {s.id === selectedId && <span className="size-1.5 rounded-full bg-accent" />}
                <ServerMeta server={s} className="max-w-[160px]" />
                <Latency latency={s.latency} className="w-16 text-[11px]" />
              </Command.Item>
            ))}
          </Group>
        )}
      </Command.List>
    </Command>
  );
}

function Group({ heading, children }: { heading: string; children: ReactNode }) {
  return (
    <Command.Group
      heading={heading}
      className="mb-1 [&_[cmdk-group-heading]]:px-2.5 [&_[cmdk-group-heading]]:pt-2.5 [&_[cmdk-group-heading]]:pb-1 [&_[cmdk-group-heading]]:text-[11px] [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group-heading]]:tracking-[0.08em] [&_[cmdk-group-heading]]:text-ink-3 [&_[cmdk-group-heading]]:uppercase"
    >
      {children}
    </Command.Group>
  );
}

function Item({
  icon: Icon,
  children,
  onSelect,
  hint,
  keywords,
  active,
}: {
  icon: LucideIcon;
  children: string;
  onSelect: () => void;
  hint?: ReactNode;
  keywords?: string[];
  active?: boolean;
}) {
  return (
    <Command.Item
      value={children}
      keywords={keywords}
      onSelect={onSelect}
      className="group flex h-9 cursor-pointer items-center gap-3 rounded-[5px] px-2.5 text-[13px] text-ink data-[selected=true]:bg-hover"
    >
      <Icon className="size-4 shrink-0 text-ink-3 group-data-[selected=true]:text-ink" strokeWidth={1.5} />
      <span className="flex-1 truncate">{children}</span>
      {active && <span className="size-1.5 rounded-full bg-accent" />}
      {hint}
    </Command.Item>
  );
}
