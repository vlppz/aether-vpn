import { useRef, useState } from "react";
import { ClipboardPaste, FileText, Download } from "lucide-react";
import { setUi, useApp } from "../store";
import { importClipboard, importText } from "../store/actions";
import { useT } from "../i18n";
import { Dialog } from "./ui/Dialog";
import { Button } from "./ui/Button";
import { Textarea } from "./ui/Input";

const SCHEMES = ["vless", "vmess", "trojan", "ss", "hysteria2", "socks", "https", "json"];

export function ImportDialog() {
  const t = useT();
  const open = useApp((s) => s.importOpen);
  const close = () => setUi({ importOpen: false });
  return (
    <Dialog open={open} onClose={close} title={t("import.title")} description={t("import.subtitle")} className="max-w-[560px]">
      {open && <ImportForm onDone={close} />}
    </Dialog>
  );
}

function ImportForm({ onDone }: { onDone: () => void }) {
  const t = useT();
  const [text, setText] = useState("");
  const [busy, setBusy] = useState<"text" | "clip" | null>(null);
  const fileRef = useRef<HTMLInputElement>(null);
  const lines = text.split(/\r?\n/).filter((l) => l.trim()).length;

  const submit = async () => {
    setBusy("text");
    const ok = await importText(text);
    setBusy(null);
    if (ok) onDone();
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="relative">
        <Textarea
          mono
          rows={8}
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder={t("import.placeholder")}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
              e.preventDefault();
              void submit();
            }
          }}
        />
        {lines > 0 && <span className="tabular absolute right-3 bottom-2.5 font-mono text-[11px] text-ink-3">{lines}</span>}
      </div>
      <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1">
        <span className="label mr-1">{t("import.supported")}</span>
        {SCHEMES.map((s) => (
          <span key={s} className="font-mono text-[11.5px] text-ink-2">
            {s}
          </span>
        ))}
      </div>
      <div className="flex items-center gap-2">
        <Button
          variant="ghost"
          icon={<ClipboardPaste />}
          loading={busy === "clip"}
          onClick={async () => {
            setBusy("clip");
            const ok = await importClipboard();
            setBusy(null);
            if (ok) onDone();
          }}
        >
          {t("import.fromClipboard")}
        </Button>
        <Button variant="ghost" icon={<FileText />} onClick={() => fileRef.current?.click()}>
          {t("import.fromFile")}
        </Button>
        <input
          ref={fileRef}
          type="file"
          accept=".txt,.json,.conf,.yaml,.yml,text/plain,application/json"
          multiple
          hidden
          onChange={async (e) => {
            const files = Array.from(e.target.files ?? []);
            const texts = await Promise.all(files.map((f) => f.text()));
            setText((prev) => [prev.trim(), ...texts].filter(Boolean).join("\n"));
            e.target.value = "";
          }}
        />
        <div className="flex-1" />
        <Button variant="primary" icon={<Download />} disabled={!text.trim()} loading={busy === "text"} onClick={submit}>
          {t("import.submit")}
        </Button>
      </div>
    </div>
  );
}
