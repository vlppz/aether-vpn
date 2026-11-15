import { useEffect, useState } from "react";
import { motion } from "motion/react";
import { Copy } from "lucide-react";
import { api, errorMessage } from "../lib/api";
import { useApp } from "../store";
import { copyServerLink } from "../store/actions";
import { useT } from "../i18n";
import { stripFlag } from "../lib/flags";
import { Dialog } from "./ui/Dialog";
import { Button } from "./ui/Button";
import { Flag } from "./ui/Flag";

export function QrDialog({ serverId, onClose }: { serverId: string | null; onClose: () => void }) {
  const t = useT();
  const server = useApp((s) => s.servers.find((x) => x.id === serverId));
  return (
    <Dialog open={!!serverId} onClose={onClose} title={t("qr.title")} description={t("qr.hint")} className="max-w-[400px]">
      {serverId && (
        <div className="flex flex-col items-center gap-4">
          {server && (
            <div className="flex w-full items-center gap-2.5">
              <Flag code={server.country} size={18} />
              <span className="truncate text-ink">{stripFlag(server.name)}</span>
            </div>
          )}
          <QrContent key={serverId} id={serverId} />
          <Button variant="secondary" className="w-full" icon={<Copy />} onClick={() => void copyServerLink(serverId)}>
            {t("qr.copy")}
          </Button>
        </div>
      )}
    </Dialog>
  );
}

function QrContent({ id }: { id: string }) {
  const [state, setState] = useState<{ svg?: string; link?: string; error?: string }>({});
  useEffect(() => {
    let alive = true;
    Promise.all([api.serverQr(id), api.exportLink(id)])
      .then(([svg, link]) => alive && setState({ svg, link }))
      .catch((e) => alive && setState({ error: errorMessage(e) }));
    return () => {
      alive = false;
    };
  }, [id]);

  return (
    <>
      <div className="relative grid aspect-square w-full max-w-[260px] place-items-center overflow-hidden rounded-md border border-line bg-white p-3">
        {state.svg ? (
          <motion.img
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.2 }}
            // rendered as an <img> so backend SVG can never run scripts
            src={`data:image/svg+xml;charset=utf-8,${encodeURIComponent(state.svg)}`}
            alt="QR"
            className="size-full [image-rendering:pixelated]"
            draggable={false}
          />
        ) : state.error ? (
          <p className="text-center text-[12px] text-[#c2412d]">{state.error}</p>
        ) : (
          <div className="size-full animate-[breathe_1.2s_ease-in-out_infinite] rounded-[4px] bg-black/5" />
        )}
      </div>
      {state.link && (
        <code data-selectable className="line-clamp-2 w-full rounded-md border border-line bg-bg px-3 py-2 font-mono text-[11px] break-all text-muted">
          {state.link}
        </code>
      )}
    </>
  );
}
