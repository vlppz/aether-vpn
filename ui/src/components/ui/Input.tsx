import { forwardRef, useState, type InputHTMLAttributes, type ReactNode, type TextareaHTMLAttributes } from "react";
import { cn } from "../../lib/cn";

const field =
  "rounded-md border border-line-strong bg-raised text-ink placeholder:text-ink-3 outline-none transition-colors duration-150 " +
  "hover:border-ink-3/60 focus:border-accent focus-visible:outline-none";

interface InputProps extends InputHTMLAttributes<HTMLInputElement> {
  icon?: ReactNode;
  trailing?: ReactNode;
  mono?: boolean;
  invalid?: boolean;
}

export const Input = forwardRef<HTMLInputElement, InputProps>(function Input({ icon, trailing, mono, invalid, className, ...rest }, ref) {
  return (
    <div className={cn("relative flex items-center", className)}>
      {icon && <span className="pointer-events-none absolute left-2.5 text-ink-3 [&>svg]:size-3.5">{icon}</span>}
      <input
        ref={ref}
        spellCheck={false}
        autoComplete="off"
        className={cn(field, "h-8 w-full text-[13px]", icon ? "pl-8" : "pl-2.5", trailing ? "pr-9" : "pr-2.5", mono && "font-mono text-[12.5px]", invalid && "border-danger")}
        {...rest}
      />
      {trailing && <span className="absolute right-1.5 flex items-center">{trailing}</span>}
    </div>
  );
});

export const Textarea = forwardRef<HTMLTextAreaElement, TextareaHTMLAttributes<HTMLTextAreaElement> & { mono?: boolean }>(function Textarea(
  { className, mono, ...rest },
  ref,
) {
  return <textarea ref={ref} spellCheck={false} className={cn(field, "w-full resize-none px-3 py-2.5 text-[13px] leading-relaxed", mono && "font-mono text-[12px]", className)} {...rest} />;
});

interface NumberInputProps {
  value: number;
  onCommit: (value: number) => void;
  min: number;
  max: number;
  label: string;
  className?: string;
}

/** Number field that only commits valid values on blur / Enter. */
export function NumberInput({ value, onCommit, min, max, label, className }: NumberInputProps) {
  const [draft, setDraft] = useState<string | null>(null);
  const text = draft ?? String(value);
  const n = Number(text);
  const valid = text !== "" && Number.isInteger(n) && n >= min && n <= max;
  const commit = () => {
    if (draft !== null && valid && n !== value) onCommit(n);
    setDraft(null);
  };
  return (
    <input
      aria-label={label}
      inputMode="numeric"
      value={text}
      onChange={(e) => setDraft(e.target.value.replace(/[^\d]/g, ""))}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
        if (e.key === "Escape") setDraft(null);
      }}
      aria-invalid={!valid}
      className={cn(field, "tabular h-8 w-24 px-2.5 text-right font-mono text-[12.5px]", !valid && "border-danger", className)}
    />
  );
}

interface TextCommitProps extends Omit<InputProps, "value" | "onChange"> {
  value: string;
  onCommit: (value: string) => void;
}

/** Text field that keeps a local draft and commits on blur / Enter. */
export function CommitInput({ value, onCommit, ...rest }: TextCommitProps) {
  const [draft, setDraft] = useState<string | null>(null);
  return (
    <Input
      {...rest}
      value={draft ?? value}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={() => {
        if (draft !== null && draft !== value) onCommit(draft);
        setDraft(null);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
        if (e.key === "Escape") setDraft(null);
      }}
    />
  );
}
