import { useEffect } from "react";
import { PAGES, setUi, useApp } from "../store";
import { importClipboard, toggleConnection } from "../store/actions";

export const isMac = typeof navigator !== "undefined" && /Mac|iPhone|iPad/i.test(navigator.platform || navigator.userAgent);
export const MOD = isMac ? "⌘" : "Ctrl";

/** true when keyboard focus is in a text field (so global shortcuts must not fire) */
export function isTyping(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el) return false;
  return el.isContentEditable || el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT";
}

function anyOverlayOpen(): boolean {
  const s = useApp.getState();
  return s.paletteOpen || s.importOpen || s.pickerOpen || !!document.querySelector('[role="dialog"], [role="menu"]');
}

/** ⌘K palette, ⌘V import, ⌘1–6 navigation, Space/Enter to toggle on Home. */
export function useGlobalShortcuts(): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = isMac ? e.metaKey : e.ctrlKey;
      const key = e.key.toLowerCase();
      if (mod && key === "k") {
        e.preventDefault();
        setUi({ paletteOpen: !useApp.getState().paletteOpen });
        return;
      }
      if (isTyping(e.target)) return;
      if (mod && key === "v" && !anyOverlayOpen()) {
        e.preventDefault();
        void importClipboard();
        return;
      }
      if (mod && /^[1-6]$/.test(e.key)) {
        e.preventDefault();
        setUi({ page: PAGES[Number(e.key) - 1] });
        return;
      }
      if ((e.key === " " || e.key === "Enter") && !mod && !e.altKey && !e.shiftKey) {
        const s = useApp.getState();
        const onButton = (e.target as HTMLElement | null)?.closest?.("button, a, [role=button], [role=radio], [role=switch], [role=menuitem], [role=option]");
        if (s.page === "home" && !onButton && !anyOverlayOpen()) {
          e.preventDefault();
          void toggleConnection();
        }
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
