import { useEffect, useState } from "react";

const STORAGE_KEY = "aether.sidebar.collapsed";
const narrowQuery = typeof window !== "undefined" ? window.matchMedia("(max-width: 1100px)") : null;

function readPref(): boolean | null {
  try {
    const v = localStorage.getItem(STORAGE_KEY);
    return v === null ? null : v === "1";
  } catch {
    return null;
  }
}

/** Collapsed state: the user's explicit choice, otherwise follows the window width. */
export function useSidebarCollapsed(): [boolean, () => void] {
  const [pref, setPref] = useState<boolean | null>(readPref);
  const [narrow, setNarrow] = useState(() => narrowQuery?.matches ?? false);
  useEffect(() => {
    const onChange = (e: MediaQueryListEvent) => setNarrow(e.matches);
    narrowQuery?.addEventListener("change", onChange);
    return () => narrowQuery?.removeEventListener("change", onChange);
  }, []);
  const collapsed = pref ?? narrow;
  const toggle = () => {
    const next = !collapsed;
    const value = next === narrow ? null : next;
    setPref(value);
    try {
      if (value === null) localStorage.removeItem(STORAGE_KEY);
      else localStorage.setItem(STORAGE_KEY, value ? "1" : "0");
    } catch {
      /* storage unavailable */
    }
  };
  return [collapsed, toggle];
}
