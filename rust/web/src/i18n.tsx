import {
  createContext,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from "react";

export type Language = "es" | "en";

const translations = {
  es: {
    "app.title": "SQLTools",
    "app.subtitle": "Migración a Rust en curso.",
    "app.language": "Idioma",
  },
  en: {
    "app.title": "SQLTools",
    "app.subtitle": "Rust migration in progress.",
    "app.language": "Language",
  },
} as const;

export type MessageKey = keyof (typeof translations)["es"];

type I18nValue = {
  language: Language;
  setLanguage: (language: Language) => void;
  t: (key: MessageKey) => string;
};

const I18nContext = createContext<I18nValue | null>(null);

export function I18nProvider({ children }: { children: ReactNode }) {
  const [language, setLanguage] = useState<Language>("es");
  const value = useMemo<I18nValue>(() => {
    const t = (key: MessageKey) => translations[language][key] ?? key;
    return { language, setLanguage, t };
  }, [language]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useTranslation(): I18nValue {
  const value = useContext(I18nContext);
  if (!value) {
    throw new Error("useTranslation must be used within I18nProvider");
  }
  return value;
}
