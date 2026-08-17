import { useEffect, useMemo, useState, type ReactNode } from "react";
import {
  I18nContext,
  detectSystemLocale,
  localeTag,
  resolveLocale,
  translate,
  type LocaleId,
  type LocalePreference,
} from "./index";

interface Props {
  preference: string | undefined;
  children: ReactNode;
}

function asPreference(value: string | undefined): LocalePreference {
  if (value === "en" || value === "zh" || value === "system") return value;
  return "system";
}

export default function I18nProvider({ preference, children }: Props) {
  const pref = asPreference(preference);
  const [systemLocale, setSystemLocale] = useState<LocaleId>(detectSystemLocale);

  useEffect(() => {
    const onChange = () => setSystemLocale(detectSystemLocale());
    window.addEventListener("languagechange", onChange);
    return () => window.removeEventListener("languagechange", onChange);
  }, []);

  const locale = pref === "system" ? systemLocale : resolveLocale(pref);

  useEffect(() => {
    document.documentElement.lang = localeTag(locale);
  }, [locale]);

  const value = useMemo(
    () => ({
      locale,
      preference: pref,
      t: (key: Parameters<typeof translate>[1], vars?: Parameters<typeof translate>[2]) =>
        translate(locale, key, vars),
    }),
    [locale, pref],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}
