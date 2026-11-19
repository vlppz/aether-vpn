import { useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { ChevronRight, Plus } from "lucide-react";
import { cn } from "../lib/cn";
import { formatDuration, splitBytes, splitSpeed } from "../lib/format";
import { countryName, stripFlag } from "../lib/flags";
import type { RoutingPreset, TunnelMode } from "../lib/types";
import { setUi, useApp, selectSelectedServer } from "../store";
import { setMode, setPreset, testRealDelay, toggleConnection } from "../store/actions";
import { useNow } from "../hooks/useNow";
import { useLang, useT } from "../i18n";
import { Armillary } from "../components/Armillary";
import { TrafficChart } from "../components/TrafficChart";
import { useWindowActive } from "../hooks/useWindowActive";
import { ServerMeta } from "../components/ServerMeta";
import { Panel } from "../components/ui/Panel";
import { Segmented } from "../components/ui/Segmented";
import { Flag } from "../components/ui/Flag";
import { Latency } from "../components/ui/Latency";
import { Button } from "../components/ui/Button";

export function Home() {
  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto">
      <div className="grid flex-1 grid-cols-[minmax(0,1fr)_minmax(300px,340px)] gap-4">
        <Hero />
        <Controls />
      </div>
      <Traffic />
    </div>
  );
}

function Hero() {
  const t = useT();
  const lang = useLang();
  const status = useApp((s) => s.status);
  const connected = status.state === "connected";
  const busy = status.state === "connecting" || status.state === "disconnecting";
  const error = status.state === "error";
  const windowActive = useWindowActive();
  const now = useNow(1000, connected && windowActive);

  return (
    <section className="flex min-w-0 flex-col items-center justify-center py-2">
      <Armillary
        state={status.state}
        onToggle={() => void toggleConnection()}
        disabled={status.state === "disconnecting"}
        className={"size-[236px] [@media(max-height:740px)]:size-[168px]"}
      />

      <div className={"mt-6 flex h-[60px] items-center [@media(max-height:740px)]:mt-3 [@media(max-height:740px)]:h-[50px]"}>
        <AnimatePresence mode="wait" initial={false}>
          <motion.h1
            key={status.state}
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.15 }}
            className={cn("font-serif text-[56px] leading-none tracking-[-0.01em] italic [@media(max-height:740px)]:text-[44px]", error ? "text-danger" : "text-ink")}
          >
            {t(`state.${status.state}`)}
          </motion.h1>
        </AnimatePresence>
      </div>

      <div className="mt-3 flex h-6 items-center">
        {connected && status.since ? (
          <span className="tabular font-mono text-[15px] tracking-[0.04em] text-ink-2">{formatDuration(now - status.since)}</span>
        ) : (
          <span className={cn("max-w-[440px] truncate text-[13px]", error ? "text-danger" : "text-ink-3")} title={status.error}>
            {error ? status.error || t("home.hintError") : busy ? t("home.hintConnecting") : t("home.hintIdle")}
          </span>
        )}
      </div>

      <div className="mt-1.5 flex h-6 items-center text-[13px]">
        {connected &&
          (status.exitIp ? (
            <span className="flex animate-[fade-in_.3s_ease-out] items-center gap-2 text-ink-2">
              <span className="label">{t("home.exitIp")}</span>
              <Flag code={status.exitCountry} size={14} />
              <span data-selectable className="font-mono text-[12.5px] text-ink">
                {status.exitIp}
              </span>
              {(status.exitCity || status.exitCountry) && (
                <span className="text-ink-3">{[status.exitCity, countryName(status.exitCountry, lang)].filter(Boolean).join(", ")}</span>
              )}
            </span>
          ) : (
            <span className="animate-[breathe_1.6s_ease-in-out_infinite] text-ink-3 italic">{t("home.detectingIp")}</span>
          ))}
      </div>
    </section>
  );
}

function Controls() {
  const t = useT();
  const server = useApp(selectSelectedServer);
  const pending = useApp((s) => (s.selectedId ? !!s.pinging[s.selectedId] : false));
  const status = useApp((s) => s.status.state);
  const mode = useApp((s) => s.settings.core.mode);
  const preset = useApp((s) => s.settings.core.routing.preset);
  const busy = status === "connecting" || status === "disconnecting";
  const connected = status === "connected";
  const [delay, setDelay] = useState<{ ms?: number; loading?: boolean }>({});

  return (
    <Panel className="flex min-w-0 flex-col self-center">
      <div className="px-5 pt-4 pb-3">
        <div className="label">{t("home.currentServer")}</div>
      </div>
      {server ? (
        <button
          type="button"
          onClick={() => setUi({ pickerOpen: true })}
          aria-label={t("home.change")}
          className="group mx-2 flex items-center gap-3 rounded-md px-3 py-2.5 text-left transition-colors hover:bg-hover focus-visible:outline-offset-[-1px]"
        >
          <Flag code={server.country} size={20} />
          <span className="min-w-0 flex-1">
            <span className="block truncate text-[15px] text-ink">{stripFlag(server.name)}</span>
            <span className="mt-0.5 flex items-center gap-2">
              <ServerMeta server={server} />
              <span className="text-ink-3" aria-hidden>
                ·
              </span>
              <Latency latency={server.latency} pending={pending} className="text-[11px]" />
            </span>
          </span>
          <ChevronRight className="size-4 text-ink-3 transition-transform group-hover:translate-x-0.5" />
        </button>
      ) : (
        <div className="flex flex-col items-start gap-3 px-5 pb-2">
          <p className="text-[13px] text-ink-2">{t("home.addServers")}</p>
          <Button variant="primary" icon={<Plus />} onClick={() => setUi({ importOpen: true })}>
            {t("servers.import")}
          </Button>
        </div>
      )}

      <div className="mt-3 border-t border-line px-5 py-4">
        <div className="mb-2.5 flex items-center justify-between">
          <span className="label">{t("home.modeLabel")}</span>
        </div>
        <Segmented<TunnelMode>
          label={t("home.modeLabel")}
          value={mode}
          onChange={(m) => void setMode(m)}
          disabled={busy}
          className="w-full [&>button]:flex-1"
          options={[
            { value: "proxy", label: t("mode.proxy") },
            { value: "tun", label: t("mode.tun") },
          ]}
        />
      </div>

      <div className="border-t border-line px-5 py-4">
        <div className="mb-2.5 label">{t("home.presetLabel")}</div>
        <Segmented<RoutingPreset>
          label={t("home.presetLabel")}
          value={preset}
          onChange={setPreset}
          className="w-full [&>button]:flex-1"
          options={[
            { value: "global", label: t("preset.global") },
            { value: "smart", label: t("preset.smart") },
            { value: "direct", label: t("preset.direct") },
          ]}
        />
      </div>

      <div className="flex items-center gap-3 border-t border-line px-5 py-3">
        <span className="label flex-1">{t("home.realDelay")}</span>
        <span className="tabular font-mono text-[12.5px] text-ink-2">{delay.ms !== undefined && connected ? t("common.ms", { n: delay.ms }) : "—"}</span>
        <Button
          size="sm"
          variant="secondary"
          disabled={!connected}
          loading={delay.loading}
          onClick={async () => {
            setDelay({ loading: true });
            const ms = await testRealDelay();
            setDelay({ ms });
          }}
        >
          {t("home.testDelay")}
        </Button>
      </div>
    </Panel>
  );
}

function Stat({ label, value, unit }: { label: string; value: string; unit: string }) {
  return (
    <div className="min-w-0 flex-1 px-5 py-3.5">
      <div className="label truncate">{label}</div>
      <div className="mt-1.5 flex items-baseline gap-1.5">
        <span className="tabular text-[22px] leading-none font-light tracking-[-0.01em] text-ink">{value}</span>
        <span className="text-[11.5px] text-ink-3">{unit}</span>
      </div>
    </div>
  );
}

function Traffic() {
  const t = useT();
  const samples = useApp((s) => s.traffic);
  const tick = useApp((s) => s.lastTick);
  const connected = useApp((s) => s.status.state === "connected");
  const down = splitSpeed(tick?.down ?? 0);
  const up = splitSpeed(tick?.up ?? 0);
  const totalDown = splitBytes(tick?.totalDown ?? 0);
  const totalUp = splitBytes(tick?.totalUp ?? 0);

  return (
    <Panel className="flex shrink-0 flex-col">
      <div className="flex divide-x divide-line border-b border-line">
        <Stat label={t("home.download")} value={down.value} unit={down.unit} />
        <Stat label={t("home.upload")} value={up.value} unit={up.unit} />
        <Stat label={t("home.downloaded")} value={totalDown.value} unit={totalDown.unit} />
        <Stat label={t("home.uploaded")} value={totalUp.value} unit={totalUp.unit} />
      </div>
      <div className="flex items-center gap-5 px-5 pt-3">
        <span className="label">{t("home.traffic")}</span>
        <span className="flex items-center gap-1.5 text-[11.5px] text-ink-2">
          <span className="h-px w-3 bg-ink" aria-hidden /> {t("home.legendDown")}
        </span>
        <span className="flex items-center gap-1.5 text-[11.5px] text-ink-2">
          <span className="h-px w-3 bg-accent" aria-hidden /> {t("home.legendUp")}
        </span>
      </div>
      <div className={"relative mx-5 mt-2 mb-3 h-[150px] [@media(max-height:740px)]:h-[84px]"}>
        <TrafficChart samples={samples} active={connected} />
        {!connected && <div className="absolute inset-0 grid place-items-center pr-16 text-[12.5px] text-ink-3 italic">{t("home.chartIdle")}</div>}
      </div>
    </Panel>
  );
}
