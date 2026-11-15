import { useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { X } from "lucide-react";
import { cn } from "../../lib/cn";
import { useT } from "../../i18n";

interface Props {
  values: string[];
  onChange: (values: string[]) => void;
  placeholder?: string;
  label: string;
  className?: string;
}

const split = (s: string) => s.split(/[\s,;]+/).map((x) => x.trim()).filter(Boolean);

export function TagInput({ values, onChange, placeholder, label, className }: Props) {
  const t = useT();
  const [draft, setDraft] = useState("");
  const add = (raw: string) => {
    const next = split(raw).filter((v) => !values.includes(v));
    if (next.length) onChange([...values, ...next]);
    setDraft("");
  };
  return (
    <div
      className={cn(
        "flex min-h-8 flex-wrap items-center gap-1 rounded-md border border-line-strong bg-raised px-1 py-[3px] transition-colors",
        "focus-within:border-accent hover:border-ink-3/60",
        className,
      )}
    >
      <AnimatePresence initial={false}>
        {values.map((v) => (
          <motion.span
            key={v}
            layout
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.12 }}
            className="inline-flex h-[22px] max-w-full items-center gap-0.5 rounded-[4px] border border-line bg-bg pr-0.5 pl-1.5 font-mono text-[11.5px] text-ink"
          >
            <span className="truncate">{v}</span>
            <button
              type="button"
              aria-label={t("tag.remove", { v })}
              onClick={() => onChange(values.filter((x) => x !== v))}
              className="grid size-[18px] place-items-center rounded-[3px] text-ink-3 hover:bg-hover hover:text-danger"
            >
              <X className="size-3" />
            </button>
          </motion.span>
        ))}
      </AnimatePresence>
      <input
        aria-label={label}
        value={draft}
        spellCheck={false}
        placeholder={values.length ? "" : placeholder}
        onChange={(e) => {
          const v = e.target.value;
          if (/[,;\s]$/.test(v)) add(v);
          else setDraft(v);
        }}
        onKeyDown={(e) => {
          if ((e.key === "Enter" || e.key === "Tab") && draft.trim()) {
            e.preventDefault();
            add(draft);
          } else if (e.key === "Backspace" && !draft && values.length) {
            onChange(values.slice(0, -1));
          }
        }}
        onBlur={() => draft.trim() && add(draft)}
        onPaste={(e) => {
          const text = e.clipboardData.getData("text");
          if (/[\s,;]/.test(text.trim())) {
            e.preventDefault();
            add(text);
          }
        }}
        className="h-[22px] min-w-24 flex-1 bg-transparent px-1.5 font-mono text-[12px] text-ink outline-none placeholder:font-sans placeholder:text-ink-3"
      />
    </div>
  );
}
