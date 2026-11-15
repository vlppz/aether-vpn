import { describe, expect, it } from "vitest";
import { translate } from ".";
import { en } from "./en";
import { ru } from "./ru";

describe("i18n", () => {
  it("has the same keys in every language", () => {
    expect(Object.keys(ru).sort()).toEqual(Object.keys(en).sort());
  });
  it("interpolates variables", () => {
    expect(translate("en", "toast.fastest", { name: "Tokyo", ms: 42 })).toBe("Fastest: Tokyo · 42 ms");
  });
  it("picks plural forms per language", () => {
    expect(translate("en", "servers.count", { n: 1 })).toBe("1 server");
    expect(translate("en", "servers.count", { n: 5 })).toBe("5 servers");
    expect(translate("ru", "servers.count", { n: 1 })).toBe("1 сервер");
    expect(translate("ru", "servers.count", { n: 3 })).toBe("3 сервера");
    expect(translate("ru", "servers.count", { n: 11 })).toBe("11 серверов");
    expect(translate("ru", "servers.count", { n: 21 })).toBe("21 сервер");
  });
});
