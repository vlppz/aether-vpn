import type { Language } from "./types";

const A = 0x1f1e6; // REGIONAL INDICATOR SYMBOL LETTER A

/** Emoji flag from an ISO 3166-1 alpha-2 code, or null for invalid input. */
export function flagEmoji(code: string | null | undefined): string | null {
  if (!code || !/^[a-z]{2}$/i.test(code)) return null;
  const up = code.toUpperCase();
  return String.fromCodePoint(A + up.charCodeAt(0) - 65, A + up.charCodeAt(1) - 65);
}

let regionNames: Intl.DisplayNames | null | undefined;

/** True when `code` is a real ISO 3166-1 alpha-2 country with a flag (e.g. "DE"), not a pseudo-code like "FL". */
export function hasFlag(code: string | null | undefined): boolean {
  if (!code || !/^[a-z]{2}$/i.test(code)) return false;
  if (regionNames === undefined) {
    try {
      regionNames = new Intl.DisplayNames(["en"], { type: "region", fallback: "none" });
    } catch {
      regionNames = null;
    }
  }
  // No Intl support: assume any well-formed pair is a flag.
  if (!regionNames) return true;
  try {
    return regionNames.of(code.toUpperCase()) !== undefined;
  } catch {
    return false;
  }
}

const nameCache = new Map<Language, Intl.DisplayNames | null>();

/** Localised country name; falls back to the upper-cased code. */
export function countryName(code: string | null | undefined, lang: Language): string {
  if (!code || !/^[a-z]{2}$/i.test(code)) return "";
  const up = code.toUpperCase();
  let dn = nameCache.get(lang);
  if (dn === undefined) {
    try {
      dn = new Intl.DisplayNames([lang], { type: "region" });
    } catch {
      dn = null;
    }
    nameCache.set(lang, dn);
  }
  try {
    return dn?.of(up) ?? up;
  } catch {
    return up;
  }
}

/** Removes a leading flag emoji (common in subscription server names) — the UI renders its own. */
export function stripFlag(name: string): string {
  return name.replace(/^(?:\p{Regional_Indicator}{2}|\u{1F3F4}[\u{E0061}-\u{E007A}]+\u{E007F})\s*/u, "").trim() || name;
}
