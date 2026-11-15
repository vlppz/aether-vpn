import { describe, expect, it } from "vitest";
import { fuzzyMatchAll, fuzzyScore } from "./fuzzy";

describe("fuzzy", () => {
  it("prefers substring and prefix matches", () => {
    const prefix = fuzzyScore("fra", "Frankfurt")!;
    const inner = fuzzyScore("fur", "Frankfurt")!;
    const sub = fuzzyScore("frf", "Frankfurt")!;
    expect(prefix).toBeGreaterThan(inner);
    expect(inner).toBeGreaterThan(sub);
  });
  it("returns null when not a subsequence", () => {
    expect(fuzzyScore("xyz", "Frankfurt")).toBeNull();
  });
  it("requires every token to match some field", () => {
    expect(fuzzyMatchAll("de vless", ["Frankfurt", "DE", "Germany", "VLESS"])).not.toBeNull();
    expect(fuzzyMatchAll("de trojan", ["Frankfurt", "DE", "Germany", "VLESS"])).toBeNull();
    expect(fuzzyMatchAll("   ", ["a"])).toBe(0);
  });
});
