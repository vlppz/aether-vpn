import { describe, expect, it } from "vitest";
import { countryName, flagEmoji, stripFlag } from "./flags";

describe("flagEmoji", () => {
  it("maps ISO codes to regional indicators", () => {
    expect(flagEmoji("DE")).toBe("🇩🇪");
    expect(flagEmoji("jp")).toBe("🇯🇵");
    expect(flagEmoji("GB")).toBe("🇬🇧");
  });
  it("rejects invalid input", () => {
    expect(flagEmoji(null)).toBeNull();
    expect(flagEmoji(undefined)).toBeNull();
    expect(flagEmoji("")).toBeNull();
    expect(flagEmoji("DEU")).toBeNull();
    expect(flagEmoji("1A")).toBeNull();
  });
});

describe("countryName", () => {
  it("localises names", () => {
    expect(countryName("DE", "en")).toBe("Germany");
    expect(countryName("de", "ru")).toBe("Германия");
  });
  it("returns empty string for missing codes", () => {
    expect(countryName(null, "en")).toBe("");
    expect(countryName("XYZ", "en")).toBe("");
  });
});

describe("stripFlag", () => {
  it("removes a leading flag emoji only", () => {
    expect(stripFlag("🇹🇷 Istanbul SS")).toBe("Istanbul SS");
    expect(stripFlag("Istanbul 🇹🇷")).toBe("Istanbul 🇹🇷");
    expect(stripFlag("🇩🇪")).toBe("🇩🇪");
    expect(stripFlag("Plain")).toBe("Plain");
  });
});
