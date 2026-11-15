import { useState } from "react";
import { Activity, Search } from "lucide-react";
import { setUi, useApp } from "../store";
import { pingAll } from "../store/actions";
import type { ServerSort } from "../store/reducers";
import { useT } from "../i18n";
import { Sheet } from "./ui/Dialog";
import { Input } from "./ui/Input";
import { Segmented } from "./ui/Segmented";
import { IconButton } from "./ui/Button";
import { ServerList } from "./ServerList";

export function ServerPicker() {
  const t = useT();
  const open = useApp((s) => s.pickerOpen);
  const close = () => setUi({ pickerOpen: false });
  return (
    <Sheet open={open} onClose={close} title={t("picker.title")}>
      {open && <PickerBody onPicked={close} />}
    </Sheet>
  );
}

function PickerBody({ onPicked }: { onPicked: () => void }) {
  const t = useT();
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<ServerSort>("default");
  const pinging = useApp((s) => Object.keys(s.pinging).length > 0);
  return (
    <>
      <div className="flex flex-col gap-2.5 px-5 pb-3">
        <Input icon={<Search />} value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("servers.search")} aria-label={t("servers.search")} />
        <div className="flex items-center gap-2">
          <Segmented
            size="sm"
            label={t("servers.sort")}
            value={sort}
            onChange={setSort}
            options={[
              { value: "default", label: t("sort.default") },
              { value: "latency", label: t("sort.latency") },
              { value: "name", label: t("sort.name") },
            ]}
          />
          <div className="flex-1" />
          <IconButton label={t("servers.pingAll")} size="sm" onClick={() => void pingAll()} disabled={pinging}>
            <Activity strokeWidth={1.5} />
          </IconButton>
        </div>
      </div>
      <ServerList compact query={query} sort={sort} onPicked={onPicked} />
    </>
  );
}
