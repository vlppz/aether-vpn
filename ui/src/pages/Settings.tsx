import { Check } from "lucide-react";
import { toast } from "sonner";
import type { CoreSettings, Language, Settings as SettingsT, Theme } from "../lib/types";
import { ACCENT_SWATCHES, resolveAccent } from "../lib/color";
import { api, openExternal } from "../lib/api";
import { isTauri } from "../lib/env";
import { cn } from "../lib/cn";
import { useApp } from "../store";
import { attempt, updateSettings } from "../store/actions";
import { useT } from "../i18n";
import { useIsDark } from "../hooks/useTheme";
import { PageHeader } from "./PageHeader";
import { Section as Panel } from "../components/ui/Panel";
import { SettingRow } from "../components/ui/SettingRow";
import { Switch } from "../components/ui/Switch";
import { Segmented } from "../components/ui/Segmented";
import { Select } from "../components/ui/Select";
import { CommitInput, NumberInput } from "../components/ui/Input";
import { Button } from "../components/ui/Button";
import { Logo } from "../components/Logo";

const set = (mut: (s: SettingsT) => void, delay?: number) => updateSettings(mut, delay);
const core = (mut: (c: CoreSettings) => void, delay?: number) => updateSettings((s) => mut(s.core), delay);

export function Settings() {
  const t = useT();
  const s = useApp((st) => st.settings);
  const platform = useApp((st) => st.platform);
  const appVersion = useApp((st) => st.appVersion);
  const coreVersion = useApp((st) => st.coreVersion);

  const toggle = (label: string, desc: string, value: boolean, onChange: (v: boolean) => void) => (
    <SettingRow label={label} description={desc}>
      <Switch label={label} checked={value} onChange={onChange} />
    </SettingRow>
  );

  return (
    <div className="flex h-full flex-col">
      <PageHeader title={t("settings.title")} />
      <div className="min-h-0 flex-1 overflow-y-auto pb-6">
        <div className="flex max-w-[760px] flex-col gap-4">
          <Section title={t("settings.general")}>
            <SettingRow label={t("settings.language")} description={t("settings.languageDesc")}>
              <Segmented<Language>
                label={t("settings.language")}
                value={s.language}
                onChange={(v) => set((x) => void (x.language = v), 0)}
                options={[
                  { value: "en", label: "English" },
                  { value: "ru", label: "Русский" },
                ]}
              />
            </SettingRow>
            <SettingRow label={t("settings.theme")} description={t("settings.themeDesc")}>
              <Segmented<Theme>
                label={t("settings.theme")}
                value={s.theme}
                onChange={(v) => set((x) => void (x.theme = v), 0)}
                options={[
                  { value: "system", label: t("theme.system") },
                  { value: "light", label: t("theme.light") },
                  { value: "dark", label: t("theme.dark") },
                ]}
              />
            </SettingRow>
            <SettingRow label={t("settings.accent")} description={t("settings.accentDesc")}>
              <AccentPicker value={s.accent} />
            </SettingRow>
            {toggle(t("settings.launchAtLogin"), t("settings.launchAtLoginDesc"), s.launchAtLogin, (v) => set((x) => void (x.launchAtLogin = v), 0))}
            {toggle(t("settings.startMinimized"), t("settings.startMinimizedDesc"), s.startMinimized, (v) => set((x) => void (x.startMinimized = v), 0))}
            {toggle(t("settings.autoConnect"), t("settings.autoConnectDesc"), s.autoConnect, (v) => set((x) => void (x.autoConnect = v), 0))}
            {toggle(t("settings.closeToTray"), t("settings.closeToTrayDesc"), s.closeToTray, (v) => set((x) => void (x.closeToTray = v), 0))}
          </Section>

          <Section title={t("settings.connection")}>
            {toggle(t("settings.setSystemProxy"), t("settings.setSystemProxyDesc"), s.setSystemProxy, (v) => set((x) => void (x.setSystemProxy = v), 0))}
            <SettingRow label={t("settings.socksPort")} description={t("settings.portDesc")}>
              <NumberInput label={t("settings.socksPort")} value={s.core.socksPort} min={1} max={65535} onCommit={(n) => core((c) => void (c.socksPort = n), 0)} />
            </SettingRow>
            <SettingRow label={t("settings.httpPort")} description={t("settings.portDesc")}>
              <NumberInput label={t("settings.httpPort")} value={s.core.httpPort} min={1} max={65535} onCommit={(n) => core((c) => void (c.httpPort = n), 0)} />
            </SettingRow>
            {toggle(t("settings.allowLan"), t("settings.allowLanDesc"), s.core.allowLan, (v) => core((c) => void (c.allowLan = v), 0))}
            {toggle(t("settings.sniffing"), t("settings.sniffingDesc"), s.core.sniffing, (v) => core((c) => void (c.sniffing = v), 0))}
          </Section>

          <Section title={t("settings.tun")}>
            <SettingRow label={t("settings.tunName")} description={t("settings.tunNameDesc")}>
              <CommitInput mono className="w-40" aria-label={t("settings.tunName")} value={s.core.tun.name} onCommit={(v) => v.trim() && core((c) => void (c.tun.name = v.trim()), 0)} />
            </SettingRow>
            <SettingRow label={t("settings.mtu")} description={t("settings.mtuDesc")}>
              <NumberInput label={t("settings.mtu")} value={s.core.tun.mtu} min={576} max={9000} onCommit={(n) => core((c) => void (c.tun.mtu = n), 0)} />
            </SettingRow>
            {toggle(t("settings.ipv6"), t("settings.ipv6Desc"), s.core.tun.ipv6, (v) => core((c) => void (c.tun.ipv6 = v), 0))}
            {platform === "macos" && (
              <SettingRow label={t("settings.service")} description={t("settings.serviceDesc")}>
                <Button
                  variant="secondary"
                  onClick={() =>
                    void attempt(async () => {
                      await api.uninstallVpnService();
                      toast.success(t("settings.serviceRemoved"));
                    })
                  }
                >
                  {t("settings.serviceUninstall")}
                </Button>
              </SettingRow>
            )}
          </Section>

          <Section title={t("settings.dns")}>
            <SettingRow label={t("settings.dnsRemote")} description={t("settings.dnsRemoteDesc")}>
              <CommitInput mono className="w-64" aria-label={t("settings.dnsRemote")} value={s.core.dns.remote} onCommit={(v) => core((c) => void (c.dns.remote = v.trim()), 0)} />
            </SettingRow>
            <SettingRow label={t("settings.dnsDirect")} description={t("settings.dnsDirectDesc")}>
              <CommitInput mono className="w-64" aria-label={t("settings.dnsDirect")} value={s.core.dns.direct} onCommit={(v) => core((c) => void (c.dns.direct = v.trim()), 0)} />
            </SettingRow>
            {toggle(t("settings.dnsTunDirect"), t("settings.dnsTunDirectDesc"), s.core.dns.tunDirect, (v) => core((c) => void (c.dns.tunDirect = v), 0))}
          </Section>

          <Section title={t("settings.core")}>
            <SettingRow label={t("settings.logLevel")} description={t("settings.logLevelDesc")}>
              <Select<CoreSettings["logLevel"]>
                label={t("settings.logLevel")}
                value={s.core.logLevel}
                onChange={(v) => core((c) => void (c.logLevel = v), 0)}
                options={(["debug", "info", "warning", "error", "none"] as const).map((v) => ({ value: v, label: t(`level.${v}`) }))}
              />
            </SettingRow>
            {toggle(t("settings.accessLog"), t("settings.accessLogDesc"), s.core.accessLog, (v) => core((c) => void (c.accessLog = v), 0))}
            <SettingRow label={t("settings.userAgent")} description={t("settings.userAgentDesc")}>
              <CommitInput
                mono
                className="w-64"
                aria-label={t("settings.userAgent")}
                placeholder={t("settings.userAgentPlaceholder")}
                value={s.userAgent ?? ""}
                onCommit={(v) => set((x) => void (x.userAgent = v.trim() || null), 0)}
              />
            </SettingRow>
          </Section>

          <Section title={t("settings.about")}>
            <div className="flex items-center gap-4 pb-4">
              <Logo size={40} className="text-ink" />
              <div className="flex-1">
                <div className="font-serif text-[30px] leading-none text-ink">Aether</div>
                <div className="mt-1.5 text-[12.5px] text-ink-2">{t("about.tagline")}</div>
              </div>
            </div>
            <SettingRow label={t("about.version")}>
              <span className="tabular font-mono text-[12.5px] text-ink-2">{appVersion || "—"}</span>
            </SettingRow>
            <SettingRow label={t("about.core")}>
              <span className={cn("tabular font-mono text-[12.5px]", coreVersion ? "text-ink-2" : "text-danger")}>{coreVersion ?? t("about.notFound")}</span>
            </SettingRow>
            <SettingRow label={t("about.backend")}>
              <span className="font-mono text-[12.5px] text-ink-2">
                {isTauri ? t("about.native") : t("about.mock")} · {platform}
              </span>
            </SettingRow>
            <div className="flex flex-wrap gap-2 border-t border-line pt-3 pb-2">
              <Button variant="secondary" onClick={() => void openExternal("https://github.com/XTLS/Xray-core")}>
                {t("about.project")}
              </Button>
              <Button variant="ghost" className="hover:text-danger" onClick={() => void attempt(() => api.quitApp())}>
                {t("about.quit")}
              </Button>
            </div>
          </Section>
        </div>
      </div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <Panel title={title} className="pt-4 pb-2">
      {children}
    </Panel>
  );
}

function AccentPicker({ value }: { value: string }) {
  const t = useT();
  const dark = useIsDark();
  const current = value.toLowerCase();
  const custom = !ACCENT_SWATCHES.some((c) => c.light === current || c.dark === current);
  return (
    <div className="flex items-center gap-2" role="radiogroup" aria-label={t("settings.accent")}>
      {ACCENT_SWATCHES.map((c) => {
        const active = c.light === current || c.dark === current;
        const color = dark ? c.dark : c.light;
        return (
          <button
            key={c.id}
            type="button"
            role="radio"
            aria-checked={active}
            aria-label={t(`accent.${c.id}`)}
            title={t(`accent.${c.id}`)}
            onClick={() => updateSettings((s) => void (s.accent = c.light), 0)}
            className={cn("grid size-6 place-items-center rounded-full transition-shadow", active ? "shadow-[0_0_0_2px_var(--surface),0_0_0_3px_var(--ink)]" : "hover:shadow-[0_0_0_2px_var(--surface),0_0_0_3px_var(--line-strong)]")}
            style={{ background: color }}
          >
            {active && <Check className="size-3" strokeWidth={2.5} style={{ color: resolveAccent(c.light, dark).fg }} />}
          </button>
        );
      })}
      <label
        title={t("settings.accentCustom")}
        className={cn(
          "relative grid size-6 cursor-pointer place-items-center rounded-full border border-dashed border-ink-3",
          custom && "border-solid border-transparent shadow-[0_0_0_2px_var(--surface),0_0_0_3px_var(--ink)]",
        )}
        style={custom ? { background: resolveAccent(value, dark).accent } : undefined}
      >
        <input
          type="color"
          aria-label={t("settings.accentCustom")}
          value={value}
          onChange={(e) => updateSettings((s) => void (s.accent = e.target.value), 250)}
          className="absolute inset-0 cursor-pointer opacity-0"
        />
        {custom ? <Check className="size-3" strokeWidth={2.5} style={{ color: resolveAccent(value, dark).fg }} /> : <span className="text-[13px] leading-none text-ink-3">+</span>}
      </label>
    </div>
  );
}
