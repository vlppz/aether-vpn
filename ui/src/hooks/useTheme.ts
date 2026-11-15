import { useEffect, useMemo, useSyncExternalStore } from "react";
import { resolveAccent, type AccentVars } from "../lib/color";
import { useApp } from "../store";

const darkQuery = typeof window !== "undefined" ? window.matchMedia("(prefers-color-scheme: dark)") : null;

function subscribeSystem(cb: () => void) {
  darkQuery?.addEventListener("change", cb);
  return () => darkQuery?.removeEventListener("change", cb);
}
const systemDark = () => darkQuery?.matches ?? false;

/** Whether the effective theme is dark. "system" follows prefers-color-scheme live. */
export function useIsDark(): boolean {
  const theme = useApp((s) => s.settings.theme);
  const sys = useSyncExternalStore(subscribeSystem, systemDark, () => false);
  return theme === "system" ? sys : theme === "dark";
}

export interface ThemeInfo extends AccentVars {
  dark: boolean;
}

export function usePalette(): ThemeInfo {
  const dark = useIsDark();
  const accent = useApp((s) => s.settings.accent);
  return useMemo(() => ({ dark, ...resolveAccent(accent, dark) }), [dark, accent]);
}

/**
 * Applies data-theme + accent variables to <html> and leaves a hint in localStorage
 * that index.html reads before first paint. Mount once.
 */
export function useThemeEffect(): void {
  const p = usePalette();
  const theme = useApp((s) => s.settings.theme);
  const ready = useApp((s) => s.ready);
  useEffect(() => {
    // before settings arrive, keep whatever index.html resolved (avoids a flash)
    if (!ready) return;
    const root = document.documentElement;
    const mode = p.dark ? "dark" : "light";
    root.dataset.theme = mode;
    root.style.setProperty("--accent", p.accent);
    root.style.setProperty("--accent-fg", p.fg);
    try {
      localStorage.setItem("aether.theme", theme);
      localStorage.setItem(`aether.vars.${mode}`, JSON.stringify({ "--accent": p.accent, "--accent-fg": p.fg }));
    } catch {
      /* storage unavailable */
    }
  }, [p, theme, ready]);
}
