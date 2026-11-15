import type { ReactNode } from "react";
import { useT } from "../i18n";
import { Dialog } from "./ui/Dialog";
import { Button } from "./ui/Button";

interface Props {
  open: boolean;
  title: ReactNode;
  body: ReactNode;
  confirmLabel: string;
  onConfirm: () => void;
  onClose: () => void;
}

export function ConfirmDialog({ open, title, body, confirmLabel, onConfirm, onClose }: Props) {
  const t = useT();
  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={title}
      className="max-w-[420px]"
      footer={
        <>
          <Button variant="ghost" onClick={onClose} data-autofocus>
            {t("common.cancel")}
          </Button>
          <Button
            variant="danger"
            onClick={() => {
              onClose();
              onConfirm();
            }}
          >
            {confirmLabel}
          </Button>
        </>
      }
    >
      <p className="text-[13.5px] text-ink-2">{body}</p>
    </Dialog>
  );
}
