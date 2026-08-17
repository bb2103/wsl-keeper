import { createContext, useContext } from "react";
import { en } from "./en";
import { zh } from "./zh";

export type LocaleId = "en" | "zh";
export type LocalePreference = "system" | LocaleId;

type Messages = typeof en;

type NestedKeyOf<T> = T extends object
  ? {
      [K in keyof T & string]: T[K] extends string
        ? K
        : `${K}.${NestedKeyOf<T[K]>}`;
    }[keyof T & string]
  : never;

export type MessageKey = NestedKeyOf<Messages>;
export type TVars = Record<string, string | number>;
export type TFunction = (key: MessageKey, vars?: TVars) => string;

const catalogs: Record<LocaleId, Messages> = {
  en,
  zh: zh as Messages,
};

export function detectSystemLocale(): LocaleId {
  const candidates = [
    navigator.language,
    ...(navigator.languages ?? []),
  ].filter(Boolean);
  return candidates.some((tag) => tag.toLowerCase().startsWith("zh"))
    ? "zh"
    : "en";
}

export function resolveLocale(preference: string | undefined): LocaleId {
  if (preference === "en" || preference === "zh") return preference;
  return detectSystemLocale();
}

export function localeTag(locale: LocaleId): string {
  return locale === "zh" ? "zh-CN" : "en-US";
}

function lookup(messages: Messages, key: MessageKey): string {
  const parts = key.split(".");
  let current: unknown = messages;
  for (const part of parts) {
    if (!current || typeof current !== "object" || !(part in current)) {
      return key;
    }
    current = (current as Record<string, unknown>)[part];
  }
  return typeof current === "string" ? current : key;
}

export function translate(
  locale: LocaleId,
  key: MessageKey,
  vars?: TVars,
): string {
  let text = lookup(catalogs[locale], key);
  if (!vars) return text;
  for (const [name, value] of Object.entries(vars)) {
    text = text.split(`{${name}}`).join(String(value));
  }
  return text;
}

export interface I18nValue {
  locale: LocaleId;
  preference: LocalePreference;
  t: TFunction;
}

export const I18nContext = createContext<I18nValue>({
  locale: "en",
  preference: "system",
  t: (key, vars) => translate("en", key, vars),
});

export function useI18n(): I18nValue {
  return useContext(I18nContext);
}

export function useT(): TFunction {
  return useI18n().t;
}
