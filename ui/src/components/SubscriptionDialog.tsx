import { useState } from "react";
import type { Subscription } from "../lib/types";
import { isHttpUrl } from "../lib/format";
import { addSubscription, saveSubscription } from "../store/actions";
import { useT } from "../i18n";
import { Dialog } from "./ui/Dialog";
import { Button } from "./ui/Button";
import { Input, NumberInput } from "./ui/Input";
import { Switch } from "./ui/Switch";

export type SubDialogState = { mode: "add" } | { mode: "edit"; sub: Subscription } | null;

export function SubscriptionDialog({ state, onClose }: { state: SubDialogState; onClose: () => void }) {
  const t = useT();
  return (
    <Dialog open={!!state} onClose={onClose} title={state?.mode === "edit" ? t("subDialog.editTitle") : t("subDialog.addTitle")} className="max-w-[520px]">
      {state && <SubForm key={state.mode === "edit" ? state.sub.id : "add"} state={state} onClose={onClose} />}
    </Dialog>
  );
}

function Field({ label, children, error }: { label: string; children: React.ReactNode; error?: string }) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="label">{label}</span>
      {children}
      {error && <span className="text-[11.5px] text-danger">{error}</span>}
    </label>
  );
}

function SubForm({ state, onClose }: { state: NonNullable<SubDialogState>; onClose: () => void }) {
  const t = useT();
  const initial: Subscription =
    state.mode === "edit" ? state.sub : { id: "", name: "", url: "", autoUpdate: true, updateIntervalHours: 24 };
  const [draft, setDraft] = useState<Subscription>(initial);
  const [touched, setTouched] = useState(false);
  const [busy, setBusy] = useState(false);
  const urlOk = isHttpUrl(draft.url);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setTouched(true);
    if (!urlOk) return;
    setBusy(true);
    const ok =
      state.mode === "add"
        ? await addSubscription(draft.url.trim(), draft.name.trim() || undefined)
        : await saveSubscription({ ...draft, url: draft.url.trim(), name: draft.name.trim() || state.sub.name, userAgent: draft.userAgent?.trim() || undefined });
    setBusy(false);
    if (ok) onClose();
  };

  return (
    <form onSubmit={submit} className="flex flex-col gap-4">
      <Field label={t("subDialog.url")} error={touched && !urlOk ? t("subDialog.invalidUrl") : undefined}>
        <Input
          mono
          value={draft.url}
          invalid={touched && !urlOk}
          placeholder="https://"
          onChange={(e) => setDraft({ ...draft, url: e.target.value })}
          onBlur={() => setTouched(true)}
        />
      </Field>
      <Field label={t("subDialog.name")}>
        <Input value={draft.name} placeholder={t("subDialog.namePlaceholder")} onChange={(e) => setDraft({ ...draft, name: e.target.value })} />
      </Field>
      {state.mode === "edit" && (
        <>
          <Field label={t("subDialog.userAgent")}>
            <Input mono value={draft.userAgent ?? ""} placeholder={t("subDialog.userAgentPlaceholder")} onChange={(e) => setDraft({ ...draft, userAgent: e.target.value })} />
          </Field>
          <div className="flex items-center gap-4 border-t border-line pt-4">
            <div className="flex-1">
              <div className="text-[13px] font-medium">{t("subDialog.autoUpdate")}</div>
              <div className="text-[12px] text-ink-3">{t("subDialog.autoUpdateDesc")}</div>
            </div>
            <Switch checked={draft.autoUpdate} onChange={(v) => setDraft({ ...draft, autoUpdate: v })} label={t("subDialog.autoUpdate")} />
          </div>
          <div className="flex items-center justify-between gap-4">
            <span className="text-[13px] text-ink">{t("subDialog.interval")}</span>
            <NumberInput
              label={t("subDialog.interval")}
              value={draft.updateIntervalHours}
              min={1}
              max={720}
              onCommit={(n) => setDraft({ ...draft, updateIntervalHours: n })}
            />
          </div>
        </>
      )}
      <div className="mt-2 flex justify-end gap-2">
        <Button variant="ghost" onClick={onClose}>
          {t("common.cancel")}
        </Button>
        <Button type="submit" variant="primary" loading={busy}>
          {busy && state.mode === "add" ? t("subDialog.fetching") : state.mode === "add" ? t("subDialog.add") : t("common.save")}
        </Button>
      </div>
    </form>
  );
}
