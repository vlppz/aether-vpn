import { describe, expect, it } from "vitest";
import { ACCENT_SWATCHES, SURFACE, adjust, contrast, ensureContrast, luminance, mix, parseHex, resolveAccent, toHex } from "./color";

describe("color", () => {
  it("parses and prints hex", () => {
    expect(parseHex("#2f5fd0")).toEqual({ r: 47, g: 95, b: 208 });
    expect(parseHex("fff")).toEqual({ r: 255, g: 255, b: 255 });
    expect(parseHex("nope")).toBeNull();
    expect(toHex({ r: 300, g: -1, b: 15.4 })).toBe("#ff000f");
  });
  it("round-trips through HSL", () => {
    expect(adjust("#2f5fd0", 0)).toBe("#2f5fd0");
    expect(adjust("#ff0000", 120)).toBe("#00ff00");
  });
  it("mixes and measures luminance/contrast", () => {
    expect(mix("#000000", "#ffffff", 0.5)).toBe("#808080");
    expect(luminance("#ffffff")).toBeCloseTo(1);
    expect(contrast("#000000", "#ffffff")).toBeCloseTo(21);
  });
  it("every swatch passes AA on its theme's surface", () => {
    for (const s of ACCENT_SWATCHES) {
      expect(contrast(s.light, SURFACE.light)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(s.dark, SURFACE.dark)).toBeGreaterThanOrEqual(4.5);
    }
  });
  it("swaps swatches per theme and fixes custom colours", () => {
    expect(resolveAccent("#2f5fd0", true).accent).toBe("#8db1ff");
    expect(resolveAccent("#8db1ff", false).accent).toBe("#2f5fd0");
    const custom = resolveAccent("#ffee00", false).accent;
    expect(contrast(custom, SURFACE.light)).toBeGreaterThanOrEqual(4.5);
    expect(contrast(resolveAccent("#200040", true).accent, SURFACE.dark)).toBeGreaterThanOrEqual(4.5);
    expect(resolveAccent("garbage", false).accent).toBe("#2f5fd0");
  });
  it("ensureContrast leaves passing colours alone", () => {
    expect(ensureContrast("#16171a", "#ffffff")).toBe("#16171a");
  });
});
