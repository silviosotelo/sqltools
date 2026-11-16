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
    row: "fila",

    sqlEditor: "Editor SQL",
    refresh: "Actualizar",
    bookmarkHint: "Clic en el margen para marcar una línea",
    objects: "Objetos",
    schema: "Esquema",
    objectType: "Tipo",
    allTypes: "Todos",
    loadObjects: "Cargar objetos",
    loadDdl: "Cargar DDL",
    object: "Objeto",
    stats: "Estadísticas",
    columns: "Columnas",
    dataType: "Tipo de dato",
    nullable: "Nulo",
    defaultValue: "Default",
    selectObject: "Seleccioná un objeto",
    name: "Nombre",
    filter: "Filtro",
    clear: "Limpiar",
    typeAll: "Todos",
    typeTable: "Tablas",
    typeView: "Vistas",
    typePackage: "Paquetes",
    typeProcedure: "Procedimientos",
    typeFunction: "Funciones",
    typeTrigger: "Triggers",
    typeSequence: "Secuencias",
    typeType: "Tipos",
    typeSynonym: "Sinónimos",
    findObject: "Buscar objeto",
    noFile: "No hay archivo abierto",
    discardChanges: "Hay cambios sin guardar. ¿Descartarlos?",
    lineEnding: "Fin de línea",
    selectionMode: "Modo de selección",
    columnSelection: "Columna",
    streamSelection: "Flujo",
    noResults: "Ejecutá una consulta para ver resultados.",
    constraints: "Restricciones",
    indexes: "Índices",
    triggers: "Triggers",
    grants: "Permisos",
    query: "Consultar",
    count: "Contar",
    copy: "Copiar nombre",
    uniqueness: "Unicidad",
    event: "Evento",
    grantee: "Beneficiario",
    privilege: "Privilegio",
    grantable: "Delegable",
    format: "Formatear",
    commandPalette: "Comandos",
    searchCommands: "Buscar comando…",
    notImplemented: "No implementado",
    workspaceSave: "Guardar workspace",
    workspaceLoad: "Abrir workspace",
    importLegacy: "Importar conexiones antiguas",
    legacyPath: "Ruta del connections/list legado (JSON)",
    checkUpdates: "Buscar actualizaciones",
    updateAvailable: "Actualización disponible",
    updateNone: "Sin actualizaciones",
    installUpdate: "Instalar actualización",
    updateProgress: "Descargando",
    updateInstalled: "Actualización instalada; reiniciá la app",

    fileSettings: "Ajustes del archivo",
    permanentSettings: "Ajustes de la aplicación",
    output: "Salida",
    noOutput: "Sin salida de DBMS_OUTPUT.",
    dbmsOutputEnabled: "DBMS_OUTPUT habilitado",
    close: "Cerrar",
    templateName: "Nombre de la plantilla",
    templateSaved: "Plantilla guardada",
    templateMissing: "No existe la plantilla",
    templateExpanded: "Plantilla insertada",
    formatterProfiles: "Perfiles de formato",
    profile: "Perfil",
    saveProfile: "Guardar perfil",
    deleteProfile: "Eliminar perfil",
    profileName: "Nombre del perfil",
    defaultProfile: "Estándar",
    profileSaved: "Perfil guardado",
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
    row: "row",

    sqlEditor: "SQL editor",
    refresh: "Refresh",
    bookmarkHint: "Click the gutter to bookmark a line",
    objects: "Objects",
    schema: "Schema",
    objectType: "Type",
    allTypes: "All",
    loadObjects: "Load objects",
    loadDdl: "Load DDL",
    object: "Object",
    stats: "Statistics",
    columns: "Columns",
    dataType: "Data type",
    nullable: "Nullable",
    defaultValue: "Default",
    selectObject: "Select an object",
    name: "Name",
    filter: "Filter",
    clear: "Clear",
    typeAll: "All",
    typeTable: "Tables",
    typeView: "Views",
    typePackage: "Packages",
    typeProcedure: "Procedures",
    typeFunction: "Functions",
    typeTrigger: "Triggers",
    typeSequence: "Sequences",
    typeType: "Types",
    typeSynonym: "Synonyms",
    findObject: "Find object",
    noFile: "No file is open",
    discardChanges: "There are unsaved changes. Discard them?",
    lineEnding: "Line endings",
    selectionMode: "Selection mode",
    columnSelection: "Column",
    streamSelection: "Stream",

    noResults: "Run a query to see results.",
    constraints: "Constraints",
    indexes: "Indexes",
    triggers: "Triggers",
    grants: "Grants",
    query: "Query",
    count: "Count",
    copy: "Copy name",
    uniqueness: "Uniqueness",
    event: "Event",
    grantee: "Grantee",
    privilege: "Privilege",
    grantable: "Grantable",
    format: "Format",
    commandPalette: "Commands",
    searchCommands: "Search command…",
    notImplemented: "Not implemented",
    workspaceSave: "Save workspace",
    workspaceLoad: "Open workspace",
    importLegacy: "Import legacy connections",
    legacyPath: "Path to the legacy connections/list (JSON)",
    checkUpdates: "Check for updates",
    updateAvailable: "Update available",
    updateNone: "No updates",
    installUpdate: "Install update",
    updateProgress: "Downloading",
    updateInstalled: "Update installed; restart the app",

    fileSettings: "File settings",
    permanentSettings: "Application settings",
    output: "Output",
    noOutput: "No DBMS_OUTPUT.",
    dbmsOutputEnabled: "DBMS_OUTPUT enabled",
    close: "Close",
    templateName: "Template name",
    templateSaved: "Template saved",
    templateMissing: "Template not found",
    templateExpanded: "Template inserted",
    formatterProfiles: "Formatter profiles",
    profile: "Profile",
    saveProfile: "Save profile",
    deleteProfile: "Delete profile",
    profileName: "Profile name",
    defaultProfile: "Standard",
    profileSaved: "Profile saved",
  },
} as const;

export type MessageKey = keyof (typeof translations)["es"];

type I18nValue = {
  language: Language;
  setLanguage: (language: Language) => void;
  t: (key: MessageKey) => string;
};

const I18nContext = createContext<I18nValue | null>(null);

const LANGUAGE_KEY = "sqlt.language";

function storedLanguage(): Language {
  try {
    return localStorage.getItem(LANGUAGE_KEY) === "en" ? "en" : "es";
  } catch {
    return "es";
  }
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [language, setLanguageState] = useState<Language>(storedLanguage);
  const setLanguage = (next: Language) => {
    setLanguageState(next);
    try {
      localStorage.setItem(LANGUAGE_KEY, next);
    } catch {
      // Storage can be unavailable; the UI still switches for this session.
    }
  };
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
