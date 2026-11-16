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
    "app.subtitle": "Editor Oracle",
    "app.language": "Idioma",
    connections: "Conexiones",
    newConnection: "Nueva conexión",
    label: "Etiqueta",
    user: "Usuario",
    password: "Contraseña",
    alias: "Alias TNS",
    connectString: "Conexión directa",
    tnsAdmin: "Carpeta TNS",
    ociDir: "Instant Client",
    mode: "Modo",
    readOnly: "Solo lectura",
    prefetch: "Prefetch",
    savePassword: "Guardar contraseña",
    save: "Guardar",
    delete: "Eliminar",
    connect: "Conectar",
    disconnect: "Desconectar",
    execute: "Ejecutar",
    cancel: "Cancelar",
    commit: "Commit",
    rollback: "Rollback",
    history: "Historial",
    explain: "Plan",
    exportCsv: "Exportar CSV",
    results: "Resultados",
    plan: "Plan de ejecución",
    noConnection: "Sin conexión",
    connected: "Conectado",
    rows: "filas",
    sqlEditor: "Editor SQL",
    refresh: "Actualizar",
    bookmarkHint: "Clic en el margen para marcar una línea",
  },
  en: {
    "app.title": "SQLTools",
    "app.subtitle": "Oracle editor",
    "app.language": "Language",
    connections: "Connections",
    newConnection: "New connection",
    label: "Label",
    user: "User",
    password: "Password",
    alias: "TNS alias",
    connectString: "Direct connection",
    tnsAdmin: "TNS folder",
    ociDir: "Instant Client",
    mode: "Mode",
    readOnly: "Read only",
    prefetch: "Prefetch",
    savePassword: "Save password",
    save: "Save",
    delete: "Delete",
    connect: "Connect",
    disconnect: "Disconnect",
    execute: "Execute",
    cancel: "Cancel",
    commit: "Commit",
    rollback: "Rollback",
    history: "History",
    explain: "Plan",
    exportCsv: "Export CSV",
    results: "Results",
    plan: "Execution plan",
    noConnection: "Not connected",
    connected: "Connected",
    rows: "rows",
    sqlEditor: "SQL editor",
    refresh: "Refresh",
    bookmarkHint: "Click the gutter to bookmark a line",
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
