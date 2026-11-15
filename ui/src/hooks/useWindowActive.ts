import { useSyncExternalStore } from "react";
import { isWindowActive, onWindowActiveChange } from "../lib/windowActive";

/** True while the window is visible and focused. */
export function useWindowActive(): boolean {
  return useSyncExternalStore(onWindowActiveChange, isWindowActive);
}
