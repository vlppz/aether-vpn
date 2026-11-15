import { useState } from "react";
import { Activity, ClipboardPaste, Plus, Search, Zap } from "lucide-react";
import { setUi, useApp } from "../store";
import { importClipboard, pingAll, selectFastest } from "../store/actions";
import type { ServerSort } from "../store/reducers";
import { MOD } from "../hooks/useGlobalShortcuts";
import { useT } from "../i18n";
import { PageHeader } from "./PageHeader";
import { ServerList } from "../components/ServerList";
import { Button } from "../components/ui/Button";
import { Input } from "../components/ui/Input";
import { Segmented } from "../components/ui/Segmented";
import { Panel } from "../components/ui/Panel";
import { EmptyState } from "../components/ui/EmptyState";
import { Kbd } from "../components/ui/Kbd";
import { EmptyServersArt } from "../components/Illustrations";

export function Servers() {
  const t = useT();
  const count = useApp((s) => s.servers.length);
  const pinging = useApp((s) => Object.keys(s.pinging).length > 0);
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<ServerSort>("default");
  const [fastBusy, setFastBusy] = useState(false);

  if (count === 0) {
    const [before, after] = t("servers.emptyHintKey").split("{key}");
    return (
      <div className="flex h-full flex-col">
        <PageHeader title={t("servers.title")} />
        <Panel className="grid flex-1 place-items-center">
          <EmptyState art={<EmptyServersArt />} title={t("servers.emptyTitle")} body={t("servers.emptyBody")}>
            <div className="flex gap-2">
              <Button variant="primary" icon={<ClipboardPaste />} onClick={() => void importClipboard()}>
                {t("servers.emptyPaste")}
              </Button>
              <Button variant="secondary" icon={<Plus />} onClick={() => setUi({ importOpen: true })}>
                {t("servers.import")}
              </Button>
            </div>
            <p className="mt-2 flex items-center gap-1.5 text-[12px] text-ink-3">
              {before}
              <Kbd>{`${MOD}V`}</Kbd>
              {after}
            </p>
            <p className="text-[12px] text-ink-3">{t("servers.emptyHintDrop")}</p>
          </EmptyState>
        </Panel>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <PageHeader title={t("servers.title")} subtitle={t("servers.count", { n: count })}>
        <Button variant="secondary" icon={<Activity />} loading={pinging} onClick={() => void pingAll()}>
          {t("servers.pingAll")}
        </Button>
        <Button
          variant="secondary"
          icon={<Zap />}
          loading={fastBusy}
          onClick={async () => {
            setFastBusy(true);
            await selectFastest();
            setFastBusy(false);
          }}
        >
          {t("servers.fastest")}
        </Button>
        <Button variant="primary" icon={<Plus />} onClick={() => setUi({ importOpen: true })}>
          {t("servers.import")}
        </Button>
      </PageHeader>
      <Panel className="flex min-h-0 flex-1 flex-col">
        <div className="flex flex-wrap items-center gap-3 border-b border-line px-3 py-2.5">
          <Input
            icon={<Search />}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Escape" && setQuery("")}
            placeholder={t("servers.search")}
            aria-label={t("servers.search")}
            className="min-w-[220px] flex-1"
          />
          <span className="label">{t("servers.sort")}</span>
          <Segmented
            label={t("servers.sort")}
            value={sort}
            onChange={setSort}
            options={[
              { value: "default", label: t("sort.default") },
              { value: "latency", label: t("sort.latency") },
              { value: "name", label: t("sort.name") },
            ]}
          />
        </div>
        <div className="flex min-h-0 flex-1 flex-col pt-1">
          <ServerList query={query} sort={sort} />
        </div>
      </Panel>
    </div>
  );
}
