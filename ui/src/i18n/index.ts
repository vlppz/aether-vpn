import { useCallback } from "react";
import { useApp } from "../store";
import type { Language } from "../lib/types";
import { en, type PluralForms } from "./en";
import { ru } from "./ru";

export type Key = keyof typeof en;
export type Vars = Record<string, string | number>;
export type TFunction = (key: Key, vars?: Vars) => string;

/** Shape every translation must satisfy: same keys, plurals stay plurals. */
export type Dict = { [K in Key]: (typeof en)[K] extends string ? string : PluralForms };

const dicts: Record<Language, Dict> = { en, ru };
const pluralRules = new Map<Language, Intl.PluralRules>();

function pluralForm(lang: Language, n: number): Intl.LDMLPluralRule {
  let pr = pluralRules.get(lang);
  if (!pr) {
    pr = new Intl.PluralRules(lang);
    pluralRules.set(lang, pr);
  }
  return pr.select(n);
}

export function translate(lang: Language, key: Key, vars?: Vars): string {
  const entry = (dicts[lang] ?? en)[key] ?? en[key];
  let text: string;
  if (typeof entry === "string") text = entry;
  else {
    const n = Number(vars?.n ?? 0);
    const form = pluralForm(lang, n);
    text = entry[form as keyof PluralForms] ?? entry.other;
  }
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m));
}

export function useLang(): Language {
  return useApp((s) => s.settings.language);
}

export function useT(): TFunction {
  const lang = useLang();
  return useCallback((key: Key, vars?: Vars) => translate(lang, key, vars), [lang]);
}

/** Non-hook translator for actions/toasts. */
export function getT(): TFunction {
  const lang = useApp.getState().settings.language;
  return (key, vars) => translate(lang, key, vars);
}
