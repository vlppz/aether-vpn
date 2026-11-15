import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { contrast } from "./color";

// Reads the theme tokens straight from index.css so the check can't drift from the source.
const css = readFileSync(new URL("../index.css", import.meta.url), "utf8");

function tokens(selector: string): Record<string, string> {
  const start = css.indexOf(selector);
  const block = css.slice(start, css.indexOf("}", start));
  const out: Record<string, string> = {};
  for (const m of block.matchAll(/--([\w-]+):\s*(#[0-9a-f]{6});/gi)) out[m[1]] = m[2];
  return out;
}

describe.each([
  ["light", tokens('[data-theme="light"] {')],
  ["dark", tokens('[data-theme="dark"] {')],
])("%s theme tokens", (_name, t) => {
  it("has the expected tokens", () => {
    for (const k of ["bg", "surface", "raised", "ink", "ink-2", "ink-3", "accent", "danger"]) expect(t[k]).toBeDefined();
  });
  it.each(["bg", "surface", "raised"])("text passes WCAG AA on %s", (bg) => {
    expect(contrast(t.ink, t[bg])).toBeGreaterThanOrEqual(7);
    expect(contrast(t["ink-2"], t[bg])).toBeGreaterThanOrEqual(4.5);
    expect(contrast(t["ink-3"], t[bg])).toBeGreaterThanOrEqual(4.5);
    expect(contrast(t.accent, t[bg])).toBeGreaterThanOrEqual(4.5);
    expect(contrast(t.danger, t[bg])).toBeGreaterThanOrEqual(4.5);
    expect(contrast(t.ok, t[bg])).toBeGreaterThanOrEqual(4.5);
    expect(contrast(t.warn, t[bg])).toBeGreaterThanOrEqual(4.5);
  });
});
