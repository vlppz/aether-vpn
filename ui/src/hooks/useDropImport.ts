import { useEffect, useState } from "react";
import { importText } from "../store/actions";

const TEXT_EXT = /\.(txt|json|conf|yaml|yml|list|sub)$/i;
const MAX_FILE = 4 * 1024 * 1024;

function hasPayload(e: DragEvent): boolean {
  const types = e.dataTransfer?.types;
  if (!types) return false;
  return types.includes("Files") || types.includes("text/plain") || types.includes("text/uri-list");
}

/**
 * Window-wide drag-and-drop import. Returns whether a drag is hovering the window.
 * (In Tauri the window must be created with `dragDropEnabled: false` for HTML5 drops to arrive.)
 */
export function useDropImport(): boolean {
  const [over, setOver] = useState(false);
  useEffect(() => {
    let depth = 0;
    // drags that start inside the window (selected text, images) are not imports
    let internal = false;
    const start = () => {
      internal = true;
    };
    const end = () => {
      internal = false;
    };
    const enter = (e: DragEvent) => {
      if (internal || !hasPayload(e)) return;
      e.preventDefault();
      depth++;
      setOver(true);
    };
    const overFn = (e: DragEvent) => {
      if (internal || !hasPayload(e)) return;
      e.preventDefault();
      if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
    };
    const leave = () => {
      if (internal) return;
      depth = Math.max(0, depth - 1);
      if (depth === 0) setOver(false);
    };
    const drop = async (e: DragEvent) => {
      if (internal) {
        internal = false;
        return;
      }
      if (!hasPayload(e)) return;
      e.preventDefault();
      depth = 0;
      setOver(false);
      const dt = e.dataTransfer;
      if (!dt) return;
      const chunks: string[] = [];
      for (const f of Array.from(dt.files)) {
        if (f.size > MAX_FILE) continue;
        if (TEXT_EXT.test(f.name) || f.type.startsWith("text/") || f.type === "application/json" || f.type === "") {
          chunks.push(await f.text());
        }
      }
      if (chunks.length === 0) chunks.push(dt.getData("text/plain") || dt.getData("text/uri-list"));
      await importText(chunks.join("\n"));
    };
    window.addEventListener("dragstart", start);
    window.addEventListener("dragend", end);
    window.addEventListener("dragenter", enter);
    window.addEventListener("dragover", overFn);
    window.addEventListener("dragleave", leave);
    window.addEventListener("drop", drop);
    return () => {
      window.removeEventListener("dragstart", start);
      window.removeEventListener("dragend", end);
      window.removeEventListener("dragenter", enter);
      window.removeEventListener("dragover", overFn);
      window.removeEventListener("dragleave", leave);
      window.removeEventListener("drop", drop);
    };
  }, []);
  return over;
}
