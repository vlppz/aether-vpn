export interface RGB {
  r: number;
  g: number;
  b: number;
}
export interface HSL {
  h: number;
  s: number;
  l: number;
}

export interface Swatch {
  id: "celestial" | "sage" | "brass" | "clay" | "slate" | "ink";
  /** canonical value stored in settings (the daylight variant) */
  light: string;
  /** night-sky variant, lighter so it reads on dark surfaces */
  dark: string;
}

/** Muted accents. Settings store `light`; the dark theme swaps in `dark`. */
export const ACCENT_SWATCHES: Swatch[] = [
  { id: "celestial", light: "#2f5fd0", dark: "#8db1ff" },
  { id: "sage", light: "#4f7d5b", dark: "#8fc29c" },
  { id: "brass", light: "#8c6a2c", dark: "#d6b46e" },
  { id: "clay", light: "#a5543a", dark: "#e59a80" },
  { id: "slate", light: "#4f596b", dark: "#a4afc2" },
  { id: "ink", light: "#16171a", dark: "#eceae4" },
];

export function parseHex(hex: string): RGB | null {
  const m = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return null;
  let h = m[1];
  if (h.length === 3) h = h.replace(/./g, (c) => c + c);
  const n = parseInt(h, 16);
  return { r: (n >> 16) & 255, g: (n >> 8) & 255, b: n & 255 };
}

export function toHex({ r, g, b }: RGB): string {
  const c = (v: number) => Math.round(Math.min(255, Math.max(0, v))).toString(16).padStart(2, "0");
  return `#${c(r)}${c(g)}${c(b)}`;
}

export function rgbToHsl({ r, g, b }: RGB): HSL {
  const rn = r / 255;
  const gn = g / 255;
  const bn = b / 255;
  const max = Math.max(rn, gn, bn);
  const min = Math.min(rn, gn, bn);
  const l = (max + min) / 2;
  if (max === min) return { h: 0, s: 0, l };
  const d = max - min;
  const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  let h: number;
  if (max === rn) h = (gn - bn) / d + (gn < bn ? 6 : 0);
  else if (max === gn) h = (bn - rn) / d + 2;
  else h = (rn - gn) / d + 4;
  return { h: h * 60, s, l };
}

export function hslToRgb({ h, s, l }: HSL): RGB {
  const hh = (((h % 360) + 360) % 360) / 360;
  if (s === 0) return { r: l * 255, g: l * 255, b: l * 255 };
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  const f = (t: number) => {
    let x = t;
    if (x < 0) x += 1;
    if (x > 1) x -= 1;
    if (x < 1 / 6) return p + (q - p) * 6 * x;
    if (x < 1 / 2) return q;
    if (x < 2 / 3) return p + (q - p) * (2 / 3 - x) * 6;
    return p;
  };
  return { r: f(hh + 1 / 3) * 255, g: f(hh) * 255, b: f(hh - 1 / 3) * 255 };
}

const clamp01 = (v: number) => Math.min(1, Math.max(0, v));

/** Rotate hue and nudge saturation / lightness. */
export function adjust(hex: string, dh: number, ds = 0, dl = 0): string {
  const rgb = parseHex(hex);
  if (!rgb) return hex;
  const hsl = rgbToHsl(rgb);
  return toHex(hslToRgb({ h: hsl.h + dh, s: clamp01(hsl.s + ds), l: clamp01(hsl.l + dl) }));
}

export function mix(a: string, b: string, t: number): string {
  const x = parseHex(a);
  const y = parseHex(b);
  if (!x || !y) return a;
  return toHex({ r: x.r + (y.r - x.r) * t, g: x.g + (y.g - x.g) * t, b: x.b + (y.b - x.b) * t });
}

/** WCAG relative luminance, 0..1 */
export function luminance(hex: string): number {
  const rgb = parseHex(hex);
  if (!rgb) return 0;
  const ch = (v: number) => {
    const c = v / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * ch(rgb.r) + 0.7152 * ch(rgb.g) + 0.0722 * ch(rgb.b);
}

/** WCAG contrast ratio between two colours (1..21). */
export function contrast(a: string, b: string): number {
  const la = luminance(a);
  const lb = luminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

/** Nudges lightness (darker on light backgrounds, lighter on dark) until `min` contrast is met. */
export function ensureContrast(hex: string, bg: string, min = 4.5): string {
  const rgb = parseHex(hex);
  if (!rgb) return hex;
  const darkBg = luminance(bg) < 0.4;
  const hsl = rgbToHsl(rgb);
  let out = toHex(rgb);
  for (let i = 0; i < 40 && contrast(out, bg) < min; i++) {
    hsl.l = clamp01(hsl.l + (darkBg ? 0.02 : -0.02));
    out = toHex(hslToRgb(hsl));
  }
  return out;
}

export interface AccentVars {
  accent: string;
  /** text/icon colour on top of a solid accent fill */
  fg: string;
}

export const SURFACE = { light: "#faf9f6", dark: "#121317" } as const;

/** Resolves the stored accent for the active theme, keeping text-level contrast on surfaces. */
export function resolveAccent(stored: string, dark: boolean): AccentVars {
  const hex = parseHex(stored) ? toHex(parseHex(stored)!) : ACCENT_SWATCHES[0].light;
  const swatch = ACCENT_SWATCHES.find((s) => s.light === hex || s.dark === hex);
  let accent: string;
  if (swatch) accent = dark ? swatch.dark : swatch.light;
  else {
    // custom colour: soften it for the night theme, then guarantee contrast
    const base = dark ? adjust(hex, 0, -0.1, 0.12) : hex;
    accent = ensureContrast(base, dark ? SURFACE.dark : SURFACE.light, 4.5);
  }
  return { accent, fg: luminance(accent) > 0.4 ? "#0b0c0f" : "#ffffff" };
}
