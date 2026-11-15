import { useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import { SortableContext, arrayMove, sortableKeyboardCoordinates, useSortable, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { GripVertical, Plus, Trash, Info } from "lucide-react";
import type { RoutingPreset, RoutingSettings, Rule, RuleKind, RuleTarget } from "../lib/types";
import { cn } from "../lib/cn";
import { useApp } from "../store";
import { setPreset, updateSettings } from "../store/actions";
import { useT, type Key } from "../i18n";
import { PageHeader } from "./PageHeader";
import { Panel, Section } from "../components/ui/Panel";
import { SettingRow } from "../components/ui/SettingRow";
import { Switch } from "../components/ui/Switch";
import { Select } from "../components/ui/Select";
import { Segmented } from "../components/ui/Segmented";
import { TagInput } from "../components/ui/TagInput";
import { Button, IconButton } from "../components/ui/Button";
import { Input } from "../components/ui/Input";

const PRESETS: { value: RoutingPreset; title: Key; desc: Key }[] = [
  { value: "global", title: "preset.global", desc: "preset.globalDesc" },
  { value: "smart", title: "preset.smart", desc: "preset.smartDesc" },
  { value: "direct", title: "preset.direct", desc: "preset.directDesc" },
];

/** Tiny diagram per preset: which share of the sky goes through the tunnel. */
function PresetGlyph({ preset, active }: { preset: RoutingPreset; active: boolean }) {
  const arc = preset === "global" ? 1 : preset === "smart" ? 0.62 : 0;
  const r = 9;
  const len = 2 * Math.PI * r;
  return (
    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" aria-hidden>
      <circle cx="12" cy="12" r={r} className="stroke-line-strong" strokeWidth="1" />
      {arc > 0 && (
        <circle
          cx="12"
          cy="12"
          r={r}
          strokeWidth="1.5"
          className={active ? "stroke-accent" : "stroke-ink-3"}
          strokeDasharray={`${len * arc} ${len}`}
          transform="rotate(-90 12 12)"
        />
      )}
      <circle cx="12" cy="12" r="2.2" className={active ? "fill-ink" : "fill-ink-3"} />
    </svg>
  );
}

const KNOWN_REGIONS = ["ru", "cn", "ir"];

const routing = (mut: (r: RoutingSettings) => void, delay?: number) => updateSettings((s) => mut(s.core.routing), delay);

export function Routing() {
  const t = useT();
  const r = useApp((s) => s.settings.core.routing);
  const [customRegion, setCustomRegion] = useState(!KNOWN_REGIONS.includes(r.region));

  return (
    <div className="flex h-full flex-col">
      <PageHeader title={t("routing.title")} subtitle={t("routing.subtitle")} />
      <div className="min-h-0 flex-1 overflow-y-auto px-1 pb-4">
        <div role="radiogroup" aria-label={t("home.presetLabel")} className="grid grid-cols-3 gap-4">
          {PRESETS.map((p) => {
            const active = r.preset === p.value;
            return (
              <button
                key={p.value}
                type="button"
                role="radio"
                aria-checked={active}
                onClick={() => setPreset(p.value)}
                className={cn(
                  "panel relative flex flex-col items-start gap-3 p-4 text-left transition-colors",
                  active ? "border-ink/40 bg-raised dark:border-ink/30" : "hover:border-line-strong",
                )}
              >
                <div className="flex w-full items-center justify-between">
                  <PresetGlyph preset={p.value} active={active} />
                  <span aria-hidden className={cn("grid size-4 place-items-center rounded-full border", active ? "border-ink" : "border-line-strong")}>
                    {active && <span className="size-2 rounded-full bg-ink" />}
                  </span>
                </div>
                <div>
                  <div className="font-serif text-[22px] leading-tight text-ink">{t(p.title)}</div>
                  <p className="mt-1 text-[12.5px] leading-snug text-ink-2">{t(p.desc)}</p>
                </div>
              </button>
            );
          })}
        </div>

        <AnimatePresence initial={false}>
          {r.preset === "smart" && (
            <motion.div initial={{ height: 0, opacity: 0 }} animate={{ height: "auto", opacity: 1 }} exit={{ height: 0, opacity: 0 }} className="overflow-hidden">
              <Panel className="mt-4 px-5 py-2">
                <SettingRow label={t("routing.region")} description={t("routing.regionDesc")}>
                  {customRegion && (
                    <Input
                      aria-label={t("routing.regionCode")}
                      placeholder={t("routing.regionCode")}
                      value={r.region}
                      maxLength={2}
                      className="w-24"
                      mono
                      onChange={(e) => routing((x) => void (x.region = e.target.value.toLowerCase().replace(/[^a-z]/g, "")))}
                    />
                  )}
                  <Select
                    label={t("routing.region")}
                    value={customRegion ? "other" : r.region}
                    onChange={(v) => {
                      if (v === "other") setCustomRegion(true);
                      else {
                        setCustomRegion(false);
                        routing((x) => void (x.region = v), 0);
                      }
                    }}
                    options={[
                      { value: "ru", label: t("region.ru") },
                      { value: "cn", label: t("region.cn") },
                      { value: "ir", label: t("region.ir") },
                      { value: "other", label: t("region.other") },
                    ]}
                  />
                </SettingRow>
              </Panel>
            </motion.div>
          )}
        </AnimatePresence>

        <Section className="mt-4" title={t("routing.options")}>
          <SettingRow label={t("routing.bypassLan")} description={t("routing.bypassLanDesc")}>
            <Switch label={t("routing.bypassLan")} checked={r.bypassLan} onChange={(v) => routing((x) => void (x.bypassLan = v), 0)} />
          </SettingRow>
          <SettingRow label={t("routing.blockAds")} description={t("routing.blockAdsDesc")}>
            <Switch label={t("routing.blockAds")} checked={r.blockAds} onChange={(v) => routing((x) => void (x.blockAds = v), 0)} />
          </SettingRow>
          <SettingRow label={t("routing.domainStrategy")} description={t("routing.domainStrategyDesc")}>
            <Select
              label={t("routing.domainStrategy")}
              value={r.domainStrategy}
              onChange={(v) => routing((x) => void (x.domainStrategy = v), 0)}
              options={(["AsIs", "IPIfNonMatch", "IPOnDemand"] as const).map((v) => ({ value: v, label: t(`ds.${v}`) }))}
            />
          </SettingRow>
        </Section>

        <Rules rules={r.rules} />
      </div>
    </div>
  );
}

function Rules({ rules }: { rules: Rule[] }) {
  const t = useT();
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }), useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }));

  const onDragEnd = (e: DragEndEvent) => {
    const { active, over } = e;
    if (!over || active.id === over.id) return;
    routing((r) => {
      const from = r.rules.findIndex((x) => x.id === active.id);
      const to = r.rules.findIndex((x) => x.id === over.id);
      r.rules = arrayMove(r.rules, from, to);
    }, 0);
  };

  const addRule = () =>
    routing((r) => {
      r.rules.push({ id: crypto.randomUUID(), enabled: true, kind: "domain", values: [], target: "proxy" });
    }, 0);

  return (
    <Section
      className="mt-4"
      title={t("routing.rules")}
      description={t("routing.rulesDesc")}
      actions={
        <Button variant="secondary" icon={<Plus />} onClick={addRule}>
          {t("routing.addRule")}
        </Button>
      }
    >
      {rules.length === 0 ? (
        <div className="rounded-md border border-dashed border-line-strong px-6 py-8 text-center">
          <div className="font-serif text-[20px] text-ink">{t("routing.noRules")}</div>
          <p className="mx-auto mt-1 max-w-sm text-[12.5px] text-ink-2">{t("routing.noRulesBody")}</p>
        </div>
      ) : (
        <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
          <SortableContext items={rules.map((r) => r.id)} strategy={verticalListSortingStrategy}>
            <ul className="-mx-2 flex flex-col">
              {rules.map((rule) => (
                <RuleRow key={rule.id} rule={rule} />
              ))}
            </ul>
          </SortableContext>
        </DndContext>
      )}
    </Section>
  );
}

const TARGET_TONE: Record<RuleTarget, string> = {
  proxy: "var(--accent)",
  direct: "var(--ok)",
  block: "var(--danger)",
};

function RuleRow({ rule }: { rule: Rule }) {
  const t = useT();
  const { attributes, listeners, setNodeRef, setActivatorNodeRef, transform, transition, isDragging } = useSortable({ id: rule.id });
  const patch = (mut: (r: Rule) => void, delay?: number) =>
    routing((r) => {
      const target = r.rules.find((x) => x.id === rule.id);
      if (target) mut(target);
    }, delay);

  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Translate.toString(transform), transition, zIndex: isDragging ? 10 : undefined }}
      className={cn(
        "relative animate-[fade-in_.25s_ease-out] rounded-md border-t px-2 py-3 first:border-t-0",
        isDragging ? "float border-line-strong" : "border-line",
        !rule.enabled && "[&>*]:opacity-55",
      )}
    >
      <div className="flex items-center gap-2.5">
        <button
          ref={setActivatorNodeRef}
          type="button"
          aria-label={t("rule.drag")}
          className="grid h-7 w-5 cursor-grab touch-none place-items-center rounded-[4px] text-ink-3 hover:bg-hover hover:text-ink active:cursor-grabbing"
          {...attributes}
          {...listeners}
        >
          <GripVertical className="size-3.5" strokeWidth={1.5} />
        </button>
        <Switch size="sm" label={t("rule.enabled")} checked={rule.enabled} onChange={(v) => patch((r) => void (r.enabled = v), 0)} />
        <Select<RuleKind>
          size="sm"
          label={t("rule.kind")}
          value={rule.kind}
          onChange={(v) => patch((r) => void (r.kind = v), 0)}
          options={(["domain", "ip", "process", "port"] as const).map((k) => ({ value: k, label: t(`rule.kind.${k}`) }))}
        />
        <Segmented<RuleTarget>
          size="sm"
          label={t("rule.target")}
          value={rule.target}
          onChange={(v) => patch((r) => void (r.target = v), 0)}
          options={(["proxy", "direct", "block"] as const).map((v) => ({ value: v, label: t(`rule.target.${v}`), dot: TARGET_TONE[v] }))}
        />
        <Input
          aria-label={t("rule.note")}
          placeholder={t("rule.note")}
          value={rule.note ?? ""}
          onChange={(e) => patch((r) => void (r.note = e.target.value || undefined), 600)}
          className="min-w-0 flex-1 [&_input]:h-7 [&_input]:border-transparent [&_input]:bg-transparent [&_input]:text-[12.5px] [&_input]:hover:border-line-strong [&_input]:focus:border-accent"
        />
        <IconButton label={t("rule.delete")} size="sm" className="hover:text-danger" onClick={() => routing((r) => void (r.rules = r.rules.filter((x) => x.id !== rule.id)), 0)}>
          <Trash strokeWidth={1.5} />
        </IconButton>
      </div>
      <div className="mt-2 pl-[30px]">
        <TagInput
          label={t(`rule.kind.${rule.kind}`)}
          values={rule.values}
          placeholder={t(`rule.ph.${rule.kind}`)}
          onChange={(values) => patch((r) => void (r.values = values), 300)}
        />
        {rule.kind === "process" && (
          <p className="mt-1.5 flex items-center gap-1.5 text-[11.5px] text-ink-3">
            <Info className="size-3" strokeWidth={1.5} /> {t("rule.processHint")}
          </p>
        )}
      </div>
    </li>
  );
}
