import { AnimatePresence, motion } from "motion/react";
import { useT } from "../i18n";
import { Logo } from "./Logo";

export function DropOverlay({ visible }: { visible: boolean }) {
  const t = useT();
  return (
    <AnimatePresence>
      {visible && (
        <motion.div
          className="pointer-events-none fixed inset-0 z-[80] grid place-items-center bg-bg/92 p-6"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.12 }}
        >
          <div className="absolute inset-4 rounded-[10px] border border-dashed border-ink-3" />
          <div className="relative flex flex-col items-center text-center text-ink">
            <Logo size={28} />
            <div className="mt-4 font-serif text-[40px] leading-none italic">{t("drop.title")}</div>
            <div className="mt-3 text-[13px] text-ink-2">{t("drop.body")}</div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
