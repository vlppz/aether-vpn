import { useState } from "react";
import { ExternalLink, LifeBuoy, Pencil, Plus, RefreshCw, Trash } from "lucide-react";
import type { Subscription } from "../lib/types";
import { cn } from "../lib/cn";
import { daysUntil, formatBytes, formatRelative, hostOf } from "../lib/format";
import { openExternal } from "../lib/api";
import { useApp } from "../store";
import { refreshAllSubscriptions, refreshSubscription, removeSubscription } from "../store/actions";
import { useNow } from "../hooks/useNow";
import { useLang, useT } from "../i18n";
import { PageHeader } from "./PageHeader";
import { Button, IconButton } from "../components/ui/Button";
import { Panel } from "../components/ui/Panel";
import { EmptyState } from "../components/ui/EmptyState";
import { EmptySubsArt } from "../components/Illustrations";
import { SubscriptionDialog, type SubDialogState } from "../components/SubscriptionDialog";
import { ConfirmDialog } from "../components/ConfirmDialog";

export function Subscriptions() {
  const t = useT();
  const subs = useApp((s) => s.subscriptions);
  const anyRefreshing = useApp((s) => Object.keys(s.refreshing).length > 0);
  const [dialog, setDialog] = useState<SubDialogState>(null);
  const [toDelete, setToDelete] = useState<Subscription | null>(null);

  return (
    <div className="flex h-full flex-col">
      <PageHeader title={t("subs.title")} subtitle={subs.length ? t("subs.count", { n: subs.length }) : undefined}>
        {subs.length > 0 && (
          <Button variant="secondary" icon={<RefreshCw className={cn(anyRefreshing && "animate-spin")} />} disabled={anyRefreshing} onClick={() => void refreshAllSubscriptions()}>
            {t("subs.refreshAll")}
          </Button>
        )}
        <Button variant="primary" icon={<Plus />} onClick={() => setDialog({ mode: "add" })}>
          {t("subs.add")}
        </Button>
      </PageHeader>

      <div className="@container min-h-0 flex-1 overflow-y-auto pb-2">
        {subs.length === 0 ? (
          <Panel className="grid h-full min-h-[360px] place-items-center">
            <EmptyState art={<EmptySubsArt />} title={t("subs.emptyTitle")} body={t("subs.emptyBody")}>
              <Button variant="primary" icon={<Plus />} onClick={() => setDialog({ mode: "add" })}>
                {t("subs.add")}
              </Button>
            </EmptyState>
          </Panel>
        ) : (
          <div className="grid grid-cols-1 gap-4 @3xl:grid-cols-2">
            {subs.map((sub) => (
              <SubCard key={sub.id} sub={sub} onEdit={() => setDialog({ mode: "edit", sub })} onDelete={() => setToDelete(sub)} />
            ))}
          </div>
        )}
      </div>

      <SubscriptionDialog state={dialog} onClose={() => setDialog(null)} />
      <ConfirmDialog
        open={!!toDelete}
        title={t("subs.deleteTitle")}
        body={t("subs.deleteBody", { name: toDelete?.name ?? "" })}
        confirmLabel={t("common.delete")}
        onConfirm={() => toDelete && void removeSubscription(toDelete.id)}
        onClose={() => setToDelete(null)}
      />
    </div>
  );
}

function Fact({ label, children, tone }: { label: string; children: React.ReactNode; tone?: "danger" }) {
  return (
    <div className="min-w-0">
      <dt className="label">{label}</dt>
      <dd className={cn("mt-1 truncate text-[13px]", tone === "danger" ? "text-danger" : "text-ink")}>{children}</dd>
    </div>
  );
}

function SubCard({ sub, onEdit, onDelete }: { sub: Subscription; onEdit: () => void; onDelete: () => void }) {
  const t = useT();
  const lang = useLang();
  const now = useNow(30_000);
  const refreshing = useApp((s) => !!s.refreshing[sub.id]);
  const serverCount = useApp((s) => s.servers.filter((x) => x.subscriptionId === sub.id).length);

  const info = sub.info;
  const used = info ? info.upload + info.download : 0;
  const unlimited = !info || info.total === 0;
  const pct = !unlimited && info ? Math.min(100, (used / info.total) * 100) : 0;
  const days = info?.expire ? daysUntil(info.expire, now) : null;
  const expiring = days !== null && days < 3;

  return (
    <Panel className="flex h-full animate-[fade-in_.25s_ease-out] flex-col">
      <div className="flex items-start gap-3 px-5 pt-4">
        <div className="min-w-0 flex-1">
          <h3 className="truncate text-[15px] font-medium text-ink">{sub.name}</h3>
          <div className="mt-0.5 truncate font-mono text-[11.5px] text-ink-3" title={sub.url}>
            {hostOf(sub.url)}
          </div>
        </div>
        <div className="-mr-2 flex items-center">
          <IconButton label={t("subs.refresh")} size="sm" onClick={() => void refreshSubscription(sub.id)} disabled={refreshing}>
            <RefreshCw className={cn(refreshing && "animate-spin")} strokeWidth={1.5} />
          </IconButton>
          <IconButton label={t("subs.edit")} size="sm" onClick={onEdit}>
            <Pencil strokeWidth={1.5} />
          </IconButton>
          <IconButton label={t("subs.delete")} size="sm" onClick={onDelete} className="hover:text-danger">
            <Trash strokeWidth={1.5} />
          </IconButton>
        </div>
      </div>

      {sub.announce && (
        <p data-selectable className="mx-5 mt-3 border-l-2 border-accent pl-3 text-[12.5px] leading-snug text-ink-2">
          {sub.announce}
        </p>
      )}

      <div className="px-5 pt-4">
        <div className="flex items-baseline justify-between gap-3">
          <span className="label">{t("subs.traffic")}</span>
          <span className="tabular font-mono text-[12px] text-ink-2">
            {unlimited
              ? `${formatBytes(used)} · ${t("subs.unlimited")}`
              : `${formatBytes(used)} / ${formatBytes(info!.total)} · ${pct.toFixed(0)}%`}
          </span>
        </div>
        <div className="mt-2 h-[3px] overflow-hidden rounded-full bg-line" role="meter" aria-valuemin={0} aria-valuemax={100} aria-valuenow={unlimited ? undefined : Math.round(pct)} aria-label={t("subs.traffic")}>
          {!unlimited && (
            <div
              className={cn("h-full rounded-full transition-[width] duration-500 ease-out", pct > 90 ? "bg-danger" : pct > 75 ? "bg-warn" : "bg-ink")}
              style={{ width: `${Math.max(pct, 1)}%` }}
            />
          )}
        </div>
      </div>

      <dl className="grid grid-cols-3 gap-4 px-5 pt-4 pb-4">
        <Fact label={t("subs.expires")} tone={expiring ? "danger" : undefined}>
          {days === null ? t("subs.noExpiry") : formatRelative(info!.expire! * 1000, now, lang)}
        </Fact>
        <Fact label={t("subs.updatedLabel")}>{sub.lastUpdated ? formatRelative(sub.lastUpdated * 1000, now, lang) : t("subs.never")}</Fact>
        <Fact label={t("nav.servers")}>
          <span className="tabular">{serverCount}</span>
          <span className="text-ink-3"> · {sub.autoUpdate ? t("subs.everyHours", { n: sub.updateIntervalHours }) : t("subs.manualUpdate")}</span>
        </Fact>
      </dl>

      {sub.lastError && (
        <div className="mx-5 mb-4 border-l-2 border-danger pl-3">
          <div className="text-[12.5px] text-danger">{t("subs.lastError")}</div>
          <div data-selectable className="truncate font-mono text-[11px] text-ink-3" title={sub.lastError}>
            {sub.lastError}
          </div>
        </div>
      )}

      {(sub.supportUrl || sub.webPageUrl) && (
        <div className="mt-auto flex gap-1 border-t border-line px-3 py-2">
          {sub.supportUrl && (
            <Button size="sm" variant="ghost" icon={<LifeBuoy strokeWidth={1.5} />} onClick={() => void openExternal(sub.supportUrl!)}>
              {t("subs.support")}
            </Button>
          )}
          {sub.webPageUrl && (
            <Button size="sm" variant="ghost" icon={<ExternalLink strokeWidth={1.5} />} onClick={() => void openExternal(sub.webPageUrl!)}>
              {t("subs.website")}
            </Button>
          )}
        </div>
      )}
    </Panel>
  );
}
