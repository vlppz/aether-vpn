import { Minus, Monitor, Moon, Search, Square, Sun, X } from "lucide-react";
import type { Theme } from "../lib/types";
import { windowControls } from "../lib/api";
import { cn } from "../lib/cn";
import { setUi, useApp } from "../store";
import { updateSettings } from "../store/actions";
import { MOD } from "../hooks/useGlobalShortcuts";
import { useT } from "../i18n";
import { IconButton } from "./ui/Button";
import { Kbd } from "./ui/Kbd";

const NEXT_THEME: Record<Theme, Theme> = { system: "light", light: "dark", dark: "system" };

export function TitleBar({ sidebarCollapsed }: { sidebarCollapsed: boolean }) {
  const t = useT();
  const platform = useApp((s) => s.platform);
  const theme = useApp((s) => s.settings.theme);
  const mac = platform === "macos";
  const ThemeIcon = theme === "system" ? Monitor : theme === "light" ? Sun : Moon;
  const themeLabel = t("titlebar.theme", { mode: t(`theme.${theme}`) });

  return (
    <header
      data-tauri-drag-region
      className={cn("flex h-10 shrink-0 items-center gap-2 pr-2", mac && sidebarCollapsed ? "pl-8" : "pl-4")}
    >
      <div data-tauri-drag-region className="flex h-full flex-1 items-center">
        <button
          type="button"
          onClick={() => setUi({ paletteOpen: true })}
          className="flex h-7 w-[min(300px,36vw)] items-center gap-2 rounded-md border border-line px-2.5 text-[12.5px] text-ink-3 transition-colors hover:border-line-strong hover:text-ink-2"
        >
          <Search className="size-3.5" strokeWidth={1.5} aria-hidden />
          <span className="flex-1 truncate text-left">{t("titlebar.search")}</span>
          <Kbd>{`${MOD}K`}</Kbd>
        </button>
      </div>
      <IconButton
        label={themeLabel}
        size="sm"
        onClick={() =>
          updateSettings((s) => {
            s.theme = NEXT_THEME[s.theme];
          }, 0)
        }
      >
        <ThemeIcon strokeWidth={1.5} />
      </IconButton>
      {!mac && (
        <div className="ml-1 flex items-center">
          <WinButton label={t("titlebar.minimize")} onClick={windowControls.minimize}>
            <Minus className="size-4" strokeWidth={1.5} />
          </WinButton>
          <WinButton label={t("titlebar.maximize")} onClick={windowControls.toggleMaximize}>
            <Square className="size-3" strokeWidth={1.5} />
          </WinButton>
          <WinButton label={t("titlebar.close")} onClick={windowControls.close} danger>
            <X className="size-4" strokeWidth={1.5} />
          </WinButton>
        </div>
      )}
    </header>
  );
}

function WinButton({ label, onClick, children, danger }: { label: string; onClick: () => void; children: React.ReactNode; danger?: boolean }) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className={cn("grid h-8 w-10 place-items-center rounded-md text-ink-2 transition-colors", danger ? "hover:bg-[#c42b1c] hover:text-white" : "hover:bg-hover hover:text-ink")}
    >
      {children}
    </button>
  );
}
