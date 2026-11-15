/** true inside the Tauri webview; false in a plain browser (mock backend). */
export const isTauri: boolean = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
