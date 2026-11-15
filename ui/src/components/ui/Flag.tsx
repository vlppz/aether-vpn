import { Globe } from "lucide-react";
import { cn } from "../../lib/cn";
import { countryName, flagEmoji } from "../../lib/flags";
import { useApp } from "../../store";
import { useLang } from "../../i18n";

interface Props {
  code: string | null | undefined;
  size?: number;
  className?: string;
}

/** Country flag. Windows can't render flag emoji, so it shows the ISO code there. */
export function Flag({ code, size = 18, className }: Props) {
  const platform = useApp((s) => s.platform);
  const lang = useLang();
  const emoji = flagEmoji(code);
  const name = countryName(code, lang);

  if (!emoji) {
    return (
      <span className={cn("inline-grid shrink-0 place-items-center text-ink-3", className)} style={{ width: size, height: size }} aria-hidden>
        <Globe style={{ width: size * 0.8, height: size * 0.8 }} strokeWidth={1.5} />
      </span>
    );
  }
  if (platform === "windows") {
    return (
      <span
        className={cn("inline-grid shrink-0 place-items-center rounded-[3px] border border-line-strong font-mono text-ink-2", className)}
        style={{ width: size * 1.35, height: size * 0.95, fontSize: size * 0.48 }}
        title={name}
        role="img"
        aria-label={name}
      >
        {code!.toUpperCase()}
      </span>
    );
  }
  return (
    <span className={cn("inline-grid shrink-0 place-items-center leading-none", className)} style={{ width: size * 1.2, height: size, fontSize: size * 0.95 }} title={name} role="img" aria-label={name}>
      {emoji}
    </span>
  );
}
