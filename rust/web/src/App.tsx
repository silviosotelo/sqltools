import { useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";

import { Editor, type EditorHandle } from "./Editor";
import { api } from "./api";
import { Icon } from "./icons";
import { TitleBar, type ThemeMode } from "./components/TitleBar";
import { ActivityBar, type SideView } from "./components/ActivityBar";
import { StatusBar } from "./components/StatusBar";
import { EditorTabs } from "./components/EditorTabs";
import { ConnectionsView, type ConnectionDraft } from "./components/ConnectionsView";
import { safeColor, validateEnvironment } from "./environment";
import { ObjectTree } from "./components/ObjectTree";
import { ActionStrip } from "./components/ActionStrip";
import { EnvBadge } from "./components/EnvBadge";
import { HistoryView } from "./components/HistoryView";
import { StatsView } from "./components/StatsView";
import { useObjectTree } from "./tree/useObjectTree";
import { ROWID_COLUMN, prepareEditable, queryByExample } from "./grid/editable";
import { objectId, type TreeGroup } from "./tree/treeModel";
import { ResultsPanel } from "./components/ResultsPanel";
import { cellText, type ObjectTab, type Panel } from "./results";
import { useTranslation } from "./i18n";
import type {
  ColumnInfo,
  ColumnMeta,
  CommandSpec,
  ConnectionConfig,
  ConstraintInfo,
  Dependency,
  FormatOptions,
  GrantInfo,
  HistoryEntry,
  IndexInfo,
  ObjectInfo,
  QueryEvent,
  StatEntry,
  TriggerInfo,
  UpdateInfo,
  Value,
  Workspace,
} from "./types";

/** One editor tab: a scratch buffer, a file, or a piece of generated DDL. */
type EditorDoc = {
  id: string;
  title: string;
  /** Stable identity for de-duplicating opened documents. */
  key: string;
  sql: string;
  cleanSql: string;
  eol: string;
  cleanEol: string;
  path: string;
};

let docSeq = 0;

const makeDoc = (title: string, sql: string, path = ""): EditorDoc => ({
  id: `doc-${++docSeq}`,
  title,
  key: path || title,
  sql,
  cleanSql: sql,
  eol: "\n",
  cleanEol: "\n",
  path,
});

const baseName = (path: string) => path.split(/[\\/]/).pop() || path;

const KEY_ALIASES: Record<string, string> = {
  pgdown: "PageDown",
  pgup: "PageUp",
  del: "Delete",
  ins: "Insert",
  esc: "Escape",
  space: "Space",
  num5: "Clear",
};

const MOD_ORDER = ["Ctrl", "Alt", "Shift"] as const;

const canonicalKey = (key: string) =>
  key.length === 1
    ? key.toUpperCase()
    : (KEY_ALIASES[key.toLowerCase()] ?? key);

/** `Ctrl+Shift+F12` → `Ctrl+Shift+F12`; `A` → `A`. */
const parseCombo = (text: string) => {
  const mods = new Set<string>();
  let key = "";
  for (const part of text.split("+").map((value) => value.trim()).filter(Boolean)) {
    const lower = part.toLowerCase();
    if (["ctrl", "control", "cmd", "meta", "command"].includes(lower)) mods.add("Ctrl");
    else if (lower === "alt") mods.add("Alt");
    else if (lower === "shift") mods.add("Shift");
    else key = part;
  }
  return [...MOD_ORDER.filter((mod) => mods.has(mod)), canonicalKey(key)].join("+");
};

/** `Ctrl+Q,A` is a chord: two combos in sequence. */
const parseSequence = (text: string) =>
  text.split(",").map((part) => parseCombo(part)).filter(Boolean);

/** Physical `Shift+1` reports `!` as `key`; the legacy map writes the digit. */
const eventKey = (event: KeyboardEvent) => {
  const digit = /^Digit(\d)$/.exec(event.code);
  return digit && !/^\d$/.test(event.key) ? digit[1] : event.key;
};

const comboFromEvent = (event: KeyboardEvent) => {
  const mods: string[] = [];
  if (event.ctrlKey || event.metaKey) mods.push("Ctrl");
  if (event.altKey) mods.push("Alt");
  if (event.shiftKey) mods.push("Shift");
  return [...mods, canonicalKey(eventKey(event))].join("+");
};

/** `["Ctrl+K,A"]` → `Ctrl+K then A`; several alternatives are joined with `/`. */
const formatKeys = (keys: string[]) =>
  keys.map((key) => key.split(",").join(" then ")).join(" / ");


type Draft = ConnectionDraft;

const objectTypes = [
  { value: "", key: "typeAll" },
  { value: "TABLE", key: "typeTable" },
  { value: "VIEW", key: "typeView" },
  { value: "PACKAGE", key: "typePackage" },
  { value: "PROCEDURE", key: "typeProcedure" },
  { value: "FUNCTION", key: "typeFunction" },
  { value: "TRIGGER", key: "typeTrigger" },
  { value: "SEQUENCE", key: "typeSequence" },
  { value: "TYPE", key: "typeType" },
  { value: "SYNONYM", key: "typeSynonym" },
] as const;

function emptyDraft(): Draft {
  return {
    id: crypto.randomUUID(),
    label: "",
    user: "",
    alias: "",
    connect: "",
    tnsAdmin: "",
    ociDir: "",
    mode: "NORMAL",
    readOnly: false,
    prefetch: 1000,
    savePassword: true,
    envLabel: "",
    envColor: "neutral",
  };
}

function fromConfig(config: ConnectionConfig): Draft {
  return {
    id: config.id,
    label: config.label,
    user: config.user,
    alias: config.alias ?? "",
    connect: config.connect ?? "",
    tnsAdmin: config.tnsAdmin ?? "",
    ociDir: config.ociDir ?? "",
    mode: config.mode,
    readOnly: config.readOnly,
    prefetch: config.prefetch,
    savePassword: config.savePassword,
    envLabel: config.environment?.label ?? "",
    envColor: safeColor(config.environment?.color ?? "neutral"),
  };
}

function toConfig(draft: Draft): ConnectionConfig {
  const nullable = (value: string) => (value.trim() ? value.trim() : null);
  return {
    id: draft.id,
    label: draft.label.trim(),
    user: draft.user.trim(),
    alias: nullable(draft.alias),
    connect: nullable(draft.connect),
    tnsAdmin: nullable(draft.tnsAdmin),
    ociDir: nullable(draft.ociDir),
    mode: draft.mode,
    readOnly: draft.readOnly,
    prefetch: draft.prefetch,
    savePassword: draft.savePassword,
    environment: draft.envLabel.trim()
      ? { label: draft.envLabel.trim(), color: draft.envColor }
      : null,
  };
}

function csvCell(value: Value): string {
  const text = cellText(value);
  if (/[",\n\r]/.test(text)) {
    return `"${text.replace(/"/g, '""')}"`;
  }
  return text;
}

export default function App() {
  const { t, language, setLanguage } = useTranslation();
  const [themeMode, setThemeMode] = useState<ThemeMode>(
    () => (localStorage.getItem("sqlt.theme") as ThemeMode) ?? "system",
  );
  const [darkTheme, setDarkTheme] = useState(false);
  const [connections, setConnections] = useState<ConnectionConfig[]>([]);
  const [connectionsLoaded, setConnectionsLoaded] = useState(false);
  const [selected, setSelected] = useState("");
  const [connected, setConnected] = useState<Record<string, string>>({});
  const [draft, setDraft] = useState<Draft>(emptyDraft);
  const [password, setPassword] = useState("");
  const [docs, setDocs] = useState<EditorDoc[]>(() => [
    makeDoc("SQL", "select * from dual"),
  ]);
  const [activeDocId, setActiveDocId] = useState("");
  const activeDoc = docs.find((doc) => doc.id === activeDocId) ?? docs[0];
  const sql = activeDoc.sql;
  const cleanSql = activeDoc.cleanSql;
  const filePath = activeDoc.path;
  const fileEol = activeDoc.eol;
  const cleanEol = activeDoc.cleanEol;

  const patchDoc = (id: string, patch: Partial<EditorDoc>) =>
    setDocs((current) =>
      current.map((doc) => (doc.id === id ? { ...doc, ...patch } : doc)),
    );
  const setSql = (next: string) => patchDoc(activeDoc.id, { sql: next });
  const setFileEol = (next: string) => patchDoc(activeDoc.id, { eol: next });

  /** Opens `text` in a tab keyed by `key` (replacing its content) and activates it. */
  const openDoc = (
    title: string,
    text: string,
    options: { key?: string; path?: string; eol?: string } = {},
  ) => {
    const key = options.key ?? options.path ?? `title:${title}`;
    const existing = docs.find((doc) => doc.key === key);
    const eol = options.eol ?? "\n";
    if (existing) {
      patchDoc(existing.id, {
        title,
        sql: text,
        cleanSql: text,
        eol,
        cleanEol: eol,
        path: options.path ?? existing.path,
      });
      setActiveDocId(existing.id);
      return existing.id;
    }
    const doc = makeDoc(title, text, options.path ?? "");
    doc.key = key;
    doc.eol = eol;
    doc.cleanEol = eol;
    setDocs((current) => [...current, doc]);
    setActiveDocId(doc.id);
    return doc.id;
  };

  const closeDoc = (id: string) => {
    const index = docs.findIndex((doc) => doc.id === id);
    if (index < 0) return;
    if (docs.length === 1) {
      const fresh = makeDoc("SQL", "");
      setDocs([fresh]);
      setActiveDocId(fresh.id);
      return;
    }
    const neighbor = docs[index - 1] ?? docs[index + 1];
    setDocs(docs.filter((doc) => doc.id !== id));
    setActiveDocId(neighbor.id);
  };

  const resetDocs = () => {
    const fresh = makeDoc("SQL", "");
    setDocs([fresh]);
    setActiveDocId(fresh.id);
  };
  const [columns, setColumns] = useState<ColumnMeta[]>([]);
  const [rows, setRows] = useState<Value[][]>([]);
  const [scrollTop, setScrollTop] = useState(0);
  const [running, setRunning] = useState(false);
  const [status, setStatus] = useState("");
  const [tables, setTables] = useState<string[]>([]);
  // Completion suggestions: every object of the owner, loaded once per connection/schema.
  const [objectSuggestions, setObjectSuggestions] = useState<
    Array<{ name: string; objectType: string }>
  >([]);
  const [historyEntries, setHistoryEntries] = useState<HistoryEntry[]>([]);
  const [plan, setPlan] = useState("");
  const [panel, setPanel] = useState<Panel>("results");
  const [schema, setSchema] = useState("");
  const [objectType, setObjectType] = useState("");
  const [selectedObject, setSelectedObject] = useState<ObjectInfo | null>(null);
  const [objectColumns, setObjectColumns] = useState<ColumnInfo[]>([]);
  const [objectTab, setObjectTab] = useState<ObjectTab>("columns");
  const [constraints, setConstraints] = useState<ConstraintInfo[]>([]);
  const [indexes, setIndexes] = useState<IndexInfo[]>([]);
  const [triggers, setTriggers] = useState<TriggerInfo[]>([]);
  const [grants, setGrants] = useState<GrantInfo[]>([]);
  const [dependencies, setDependencies] = useState<Dependency[]>([]);
  const [invalidCount, setInvalidCount] = useState(0);
  const [menu, setMenu] = useState<{
    object: ObjectInfo;
    x: number;
    y: number;
  } | null>(null);
  const [activeCell, setActiveCell] = useState<{
    column: string;
    value: string;
    row: number;
  } | null>(null);
  const [cellView, setCellView] = useState<{
    column: string;
    value: string;
    row: number;
  } | null>(null);
  // Grid editing: only a single-table select is editable; the ROWID travels next to the rows.
  const [editTable, setEditTable] = useState<string | null>(null);
  const [rowIds, setRowIds] = useState<string[]>([]);
  const [pendingEdits, setPendingEdits] = useState<
    Array<{ row: number; rowId: string; column: string; oldValue: string | null; newValue: string | null }>
  >([]);
  const [editValue, setEditValue] = useState("");
  const [qbe, setQbe] = useState("");
  const [helpOpen, setHelpOpen] = useState(false);
  const [showToolbar, setShowToolbar] = useState(true);
  // null = unknown (never queried, or the last query failed).
  // The flag belongs to one session: keeping the id next to the value stops a stale flag from being
  // shown after the user switches connection.
  const [transactionFlag, setTransactionFlag] = useState<{ id: string; pending: boolean } | null>(null);
  const [transactionTick, setTransactionTick] = useState(0);
  const objectsFilterRef = useRef<HTMLInputElement | null>(null);
  const dataPanelRef = useRef<HTMLElement | null>(null);
  const profileDetailsRef = useRef<HTMLDetailsElement | null>(null);
  const profileSelectRef = useRef<HTMLSelectElement | null>(null);

  /** 0 = objects filter, 1 = editor, 2 = results. */
  const lastPane = useRef(1);
  useEffect(() => {
    const onFocus = (event: FocusEvent) => {
      const target = event.target as HTMLElement | null;
      if (!target) return;
      if (objectsFilterRef.current?.contains(target)) lastPane.current = 0;
      else if (dataPanelRef.current?.contains(target)) lastPane.current = 2;
      else if (target.closest(".cm-editor")) lastPane.current = 1;
    };
    document.addEventListener("focusin", onFocus);
    return () => document.removeEventListener("focusin", onFocus);
  }, []);

  /**
   * A dialog (or the palette) holds the focus when a command runs, so the
   * focused element says nothing about the pane the user was in; `focusin`
   * keeps the last real pane. 0 = objects filter, 1 = editor, 2 = results.
   */
  const activePane = () => lastPane.current;

  const focusPane = (index: number) => {
    const next = ((index % 3) + 3) % 3;
    lastPane.current = next;
    if (next === 0) focusObjectsFilter();
    else if (next === 1) editorRef.current?.focus();
    else dataPanelRef.current?.focus();
  };
  const [stats, setStats] = useState<StatEntry[]>([]);
  const [output, setOutput] = useState<string[]>([]);
  const [scriptErrors, setScriptErrors] = useState<
    { message: string; offset: number }[]
  >([]);
  const [errorIndex, setErrorIndex] = useState(-1);
  const bufferVersion = useRef(0);
  const [outlineOpen, setOutlineOpen] = useState(false);
  const [outline, setOutline] = useState<{ line: number; label: string }[]>(
    [],
  );
  const [openFilesOpen, setOpenFilesOpen] = useState(false);
  const [newExtendedOpen, setNewExtendedOpen] = useState(false);
  const [filePanelOpen, setFilePanelOpen] = useState(false);
  const [findInFilesOpen, setFindInFilesOpen] = useState(false);
  const [findQuery, setFindQuery] = useState("");
  const [profiles, setProfiles] = useState<string[]>([]);
  const [profile, setProfile] = useState("");
  const [profileOptions, setProfileOptions] = useState<FormatOptions | null>(
    null,
  );
  const [showStatus, setShowStatus] = useState(true);
  const [sideView, setSideView] = useState<SideView>("objects");
  // Mirrors `sideView` for async handlers that must not switch the view after the user moved on.
  const sideViewRef = useRef<SideView>("objects");
  sideViewRef.current = sideView;
  // Bumped every time the user opens the connections view: a save or connect that finishes later
  // may only close the view if the user has not left and come back since it started.
  const connectionsVisit = useRef(0);
  const openConnections = () => {
    connectionsVisit.current += 1;
    setSideView("connections");
  };
  const closeConnectionsFrom = (visit: number) => {
    if (connectionsVisit.current === visit)
      setSideView((view) => (view === "connections" ? "objects" : view));
  };
  const pendingObjectsFocus = useRef(false);
  const focusObjectsFilter = () => {
    if (objectsFilterRef.current) objectsFilterRef.current.focus();
    else {
      pendingObjectsFocus.current = true;
      setSideView("objects");
    }
  };
  const firstSideViewRender = useRef(true);
  useEffect(() => {
    if (firstSideViewRender.current) {
      firstSideViewRender.current = false;
      return;
    }
    if (sideView === "objects" && pendingObjectsFocus.current) {
      pendingObjectsFocus.current = false;
      objectsFilterRef.current?.focus();
      return;
    }
    // A control of the previous view was unmounted with the focus: land on this view's activity button.
    const active = document.activeElement;
    if (!active || active === document.body) {
      document.querySelector<HTMLElement>(".activity .act[aria-pressed='true']")?.focus();
    }
  }, [sideView]);
  const [cursor, setCursor] = useState<{ line: number; col: number } | null>(
    null,
  );
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [installing, setInstalling] = useState(false);
  const [haltOnError, setHaltOnErrorState] = useState(() => {
    try {
      return localStorage.getItem("sqlt.haltOnError") !== "false";
    } catch {
      return true;
    }
  });
  const [splitPercent, setSplitPercentState] = useState(() => {
    try {
      const stored = Number(localStorage.getItem("sqlt.splitPercent"));
      return Number.isFinite(stored) && stored >= 20 && stored <= 85
        ? stored
        : 60;
    } catch {
      return 60;
    }
  });
  const [splitterActive, setSplitterActive] = useState(false);
  const workspaceRef = useRef<HTMLDivElement | null>(null);
  const splitterRef = useRef<HTMLDivElement | null>(null);
  const clampSplit = (value: number) => Math.max(20, Math.min(85, value));
  const setSplitPercentValue = (value: number) => {
    const next = clampSplit(value);
    setSplitPercentState(next);
    try {
      localStorage.setItem("sqlt.splitPercent", String(next));
    } catch {
      // Storage may be unavailable; the splitter still works this session.
    }
  };
  const nudgeSplit = (delta: number) => {
    const next = clampSplit(splitPercent + delta);
    setSplitPercentValue(next);
    setStatus(`${t("splitter")}: ${Math.round(next)}%`);
  };
  const [substitution, setSubstitutionState] = useState(() => {
    try {
      return localStorage.getItem("sqlt.substitution") === "true";
    } catch {
      return false;
    }
  });
  const setSubstitution = (value: boolean) => {
    setSubstitutionState(value);
    try {
      localStorage.setItem("sqlt.substitution", String(value));
    } catch {
      // Storage may be unavailable; the toggle still applies this session.
    }
  };
  const setHaltOnError = (value: boolean) => {
    setHaltOnErrorState(value);
    try {
      localStorage.setItem("sqlt.haltOnError", String(value));
    } catch {
      // Storage may be unavailable; the toggle still applies this session.
    }
  };
  const [snippetsOpen, setSnippetsOpen] = useState(false);
  const [snippetNames, setSnippetNames] = useState<string[]>([]);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [settingsSection, setSettingsSection] = useState<"file" | "app">("app");
  const [commands, setCommands] = useState<CommandSpec[]>([]);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [paletteQuery, setPaletteQuery] = useState("");
  const [paletteIndex, setPaletteIndex] = useState(0);
  // Read by the window-level accelerator, which cannot see the latest state inside a stable listener.
  const paletteOpenRef = useRef(false);
  paletteOpenRef.current = paletteOpen;
  useEffect(() => {
    if (!paletteOpen) {
      setPaletteQuery("");
      setPaletteIndex(0);
    }
  }, [paletteOpen]);

  const isConnected = Boolean(selected && connected[selected]);
  // Oracle is asked only when no statement is running (it would wait for the same session lock), after a run
  // ends and after commit/rollback. A failed query leaves the state unknown instead of guessing.
  useEffect(() => {
    if (!isConnected || running) {
      setTransactionFlag(null);
      return;
    }
    let current = true;
    const id = selected;
    api
      .transactionPending(id)
      .then((pending) => {
        // null = the session was busy; keep the previous value instead of inventing "no transaction".
        if (!current || pending === null) return;
        setTransactionFlag({ id, pending });
      })
      .catch(() => current && setTransactionFlag(null));
    return () => {
      current = false;
    };
  }, [isConnected, running, selected, transactionTick]);
  const transactionPending = transactionFlag?.id === selected ? transactionFlag.pending : null;
  useEffect(() => {
    if (!isConnected) {
      setInvalidCount(0);
      return;
    }
    let current = true;
    const timer = setTimeout(() => {
      api
        .listInvalidObjects(selected, schema.trim() || null)
        .then((list) => current && setInvalidCount(list.length))
        .catch(() => current && setInvalidCount(0));
    }, 400);
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [isConnected, selected, schema]);
  const environment = connections.find((config) => config.id === selected)?.environment ?? null;
  const treeGroups: TreeGroup[] = useMemo(
    () =>
      objectTypes
        .filter((type) => type.value !== "")
        .map((type) => ({ type: type.value, label: t(type.key) })),
    [t],
  );
  const tree = useObjectTree(
    isConnected ? selected : null,
    schema,
    treeGroups,
    selectedObject ? objectId(selectedObject.objectType, selectedObject.name) : undefined,
  );

  const refresh = async () => {
    setConnections(await api.listConnections());
    setConnectionsLoaded(true);
  };

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const dark =
        themeMode === "dark" || (themeMode === "system" && media.matches);
      document.documentElement.dataset.theme = dark ? "dark" : "light";
      setDarkTheme(dark);
      localStorage.setItem("sqlt.theme", themeMode);
    };
    apply();
    if (themeMode !== "system") return;
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [themeMode]);

  useEffect(() => {
    // Only after the list has loaded: the first render always has zero connections.
    if (connectionsLoaded && connections.length === 0) openConnections();
  }, [connectionsLoaded, connections.length]);

  // Changing the buffer invalidates the offsets of the last run's errors and
  // makes late events from that run unusable.
  useEffect(() => {
    bufferVersion.current += 1;
    setScriptErrors([]);
    setErrorIndex(-1);
  }, [sql]);

  useEffect(() => {
    void api.listCommands().then(setCommands);
    void api.listFormatProfiles().then(setProfiles);
  }, []);

  // Every accelerator from the original menu spec (`Ctrl+Q,A` chords included)
  // runs its command; the ones the editor already handled are skipped because
  // CodeMirror calls `preventDefault`.
  useEffect(() => {
    const single = new Map<string, string>();
    const chord = new Map<string, string>();
    const prefixes = new Set<string>();
    for (const command of commands) {
      if (!command.implemented) continue;
      for (const keys of command.keys) {
        const combos = parseSequence(keys);
        if (combos.length === 1) single.set(combos[0], command.name);
        else if (combos.length === 2) {
          chord.set(`${combos[0]}>${combos[1]}`, command.name);
          prefixes.add(combos[0]);
        }
      }
    }
    let pending: string | null = null;
    let timer: number | undefined;
    const clearPending = () => {
      if (!pending) return;
      pending = null;
      if (timer) window.clearTimeout(timer);
      setStatus("");
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      // With the palette open its input owns the keyboard: Ctrl+A, Ctrl+V and typing are editing keys, not
      // commands. (The palette's own handler deals with arrows, Enter and Escape.)
      if (paletteOpenRef.current) return;
      if (event.key === "Escape" && pending) {
        event.preventDefault();
        clearPending();
        return;
      }
      const combo = comboFromEvent(event);
      if (pending) {
        const name = chord.get(`${pending}>${combo}`);
        pending = null;
        if (timer) window.clearTimeout(timer);
        setStatus("");
        if (name) {
          event.preventDefault();
          commandRunner.current(name);
          return;
        }
      }
      if (prefixes.has(combo) && !single.has(combo)) {
        pending = combo;
        if (timer) window.clearTimeout(timer);
        setStatus(`${t("chordPending")} ${combo}`);
        timer = window.setTimeout(() => {
          pending = null;
          setStatus("");
        }, 1500);
        event.preventDefault();
        return;
      }
      const name = single.get(combo);
      if (name) {
        event.preventDefault();
        commandRunner.current(name);
      }
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("blur", clearPending);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("blur", clearPending);
      if (timer) window.clearTimeout(timer);
    };
  }, [commands, t]);

  // Dropping a `.sql` file on the window opens it, like the original.
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void (async () => {
      const stop = await getCurrentWebview().onDragDropEvent((event) => {
        const payload = event.payload as { type: string; paths?: string[] };
        if (payload.type !== "drop") return;
        const path = (payload.paths ?? []).find((candidate) =>
          candidate.toLowerCase().endsWith(".sql"),
        );
        if (!path) return;
        void api
          .readSqlFile(path)
          .then((file) => {
            openDoc(baseName(path), file.text, { path, eol: file.eol });
            setStatus(path);
          })
          .catch((error) => setStatus(String(error)));
      });
      if (disposed) stop();
      else unlisten = stop;
    })();
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (
        (event.ctrlKey || event.metaKey) &&
        event.shiftKey &&
        event.key.toLowerCase() === "p"
      ) {
        event.preventDefault();
        // Closing must never leave the focus on <body>: it goes back to the editor.
        if (paletteOpenRef.current) {
          setPaletteOpen(false);
          editorRef.current?.focus();
        } else {
          setPaletteOpen(true);
        }
      }
      if (event.key === "Escape" && paletteOpenRef.current) {
        setPaletteOpen(false);
        editorRef.current?.focus();
        return;
      }
      // While the palette is open every other keystroke belongs to its input (Ctrl+A, Ctrl+V, typing...),
      // not to the editor command map.
      if (paletteOpenRef.current) return;
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    if (!selected && connections[0]) {
      selectConnection(connections[0].id);
    }
  }, [connections, selected]);

  const selectConnection = (id: string) => {
    setSelected(id);
    const config = connections.find((item) => item.id === id);
    if (config) {
      setDraft(fromConfig(config));
      setPassword("");
    }
    setSelectedObject(null);
    setObjectColumns([]);
    setConstraints([]);
    setIndexes([]);
    setTriggers([]);
    setGrants([]);
    setDependencies([]);
    setObjectTab("columns");
    setMenu(null);
    setStats([]);
  };

  const columnCache = useRef(new Map<string, string[]>());
  const editorRef = useRef<EditorHandle | null>(null);
  const printScriptRef = useRef<HTMLPreElement | null>(null);
  const commandRunner = useRef<(name: string) => void>(() => {});

  const handleSideView = (view: SideView) => {
    if (view === "history") void showHistory();
    else if (view === "stats") void showStats();
    else if (view === "connections") openConnections();
    else setSideView(view);
  };

  const loadSuggestions = async (connectionId: string) => {
    try {
      const list = await api.suggestObjects(connectionId, schema.trim() || null);
      setObjectSuggestions(
        list.map((object) => ({ name: object.name, objectType: object.objectType })),
      );
    } catch {
      setObjectSuggestions([]);
    }
  };

  useEffect(() => {
    if (!isConnected) {
      setObjectSuggestions([]);
      return;
    }
    let current = true;
    const timer = setTimeout(() => {
      if (current) void loadSuggestions(selected);
    }, 600);
    return () => {
      current = false;
      clearTimeout(timer);
    };
  }, [isConnected, selected, schema]);

  const loadColumnsFor = async (table: string) => {
    // The editor hands over the name as the user typed it; Oracle stores unquoted names in upper case,
    // so `a11.` must look for `A11` or the column query finds nothing.
    table = table
      .split(".")
      .map((part) => (/^[A-Za-z][A-Za-z0-9_$#]*$/.test(part) ? part.toUpperCase() : part))
      .join(".");
    const parts = table.replace(/"/g, "").split(".");
    const name = parts.pop() ?? table;
    const owner = parts.pop() ?? (schema.trim() || null);
    const key = `${owner ?? ""}|${name}`;
    const cached = columnCache.current.get(key);
    if (cached) return cached;
    const columns = await api.listColumns(selected, owner, name);
    const names = columns.map((column) => column.name);
    columnCache.current.set(key, names);
    return names;
  };
  const update = <K extends keyof Draft>(key: K, value: Draft[K]) => {
    setDraft((current) => ({ ...current, [key]: value }));
  };

  const save = async () => {
    const visit = connectionsVisit.current;
    if (draft.envLabel.trim()) {
      const env = validateEnvironment({ label: draft.envLabel, color: draft.envColor });
      if (!env.ok) {
        setStatus(t("envInvalid"));
        return;
      }
    }
    try {
      await api.saveConnection(toConfig(draft), password);
      setPassword("");
      await refresh();
      setSelected(draft.id);
      closeConnectionsFrom(visit);
      setStatus(t("save"));
    } catch (error) {
      setStatus(String(error));
    }
  };

  const newConnection = () => {
    setSelected("");
    setDraft(emptyDraft());
    setPassword("");
  };

  const remove = async () => {
    if (!selected) return;
    if (!window.confirm(`${t("delete")} ${draft.label || selected}?`)) return;
    try {
      await api.deleteConnection(selected);
      setConnected((current) => {
        const next = { ...current };
        delete next[selected];
        return next;
      });
      setSelected("");
      setDraft(emptyDraft());
      await refresh();
    } catch (error) {
      setStatus(String(error));
    }
  };

  const connect = async () => {
    if (!selected) {
      setStatus(t("noConnection"));
      return;
    }
    const visit = connectionsVisit.current;
    try {
      const version = await api.connect(selected);
      setConnected((current) => ({ ...current, [selected]: version }));
      setStatus(`${t("connected")}: ${version}`);
      setTables(await api.listTableNames(selected));
      closeConnectionsFrom(visit);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const disconnect = async () => {
    if (!selected) return;
    try {
      await api.disconnect(selected);
      setConnected((current) => {
        const next = { ...current };
        delete next[selected];
        return next;
      });
      setTables([]);
        setSelectedObject(null);
      setObjectColumns([]);
      setStats([]);
      setStatus(t("disconnect"));
    } catch (error) {
      setStatus(String(error));
    }
  };

  const substituteVariables = (
    text: string,
    offset: number | null,
  ): { text: string; offset: number | null } | null => {
    const values = new Map<string, string>();
    let out = "";
    let inQuote = false;
    let mapped: number | null = null;
    for (let index = 0; index < text.length; index += 1) {
      if (offset !== null && mapped === null && index >= offset) {
        mapped = out.length;
      }
      const char = text[index];
      if (char === "'") {
        if (inQuote && text[index + 1] === "'") {
          out += "''";
          index += 1;
          continue;
        }
        inQuote = !inQuote;
        out += char;
        continue;
      }
      if (!inQuote && char === "&") {
        let cursor = index + 1;
        if (text[cursor] === "&") cursor += 1;
        let end = cursor;
        while (end < text.length && /[A-Za-z0-9_]/.test(text[end])) end += 1;
        if (end > cursor) {
          const name = text.slice(cursor, end);
          let value = values.get(name);
          if (value === undefined) {
            const answer = window.prompt(`${t("variable")} ${name}`);
            if (answer === null) return null;
            value = answer;
            values.set(name, value);
          }
          if (
            offset !== null &&
            mapped === null &&
            offset >= index &&
            offset <= end
          ) {
            mapped = out.length;
          }
          out += value;
          index = end - 1;
          continue;
        }
      }
      out += char;
    }
    if (offset !== null && mapped === null) mapped = out.length;
    return { text: out, offset: mapped };
  };

  const run = async (text = sql, offset: number | null = null) => {
    if (!isConnected) {
      setStatus(t("noConnection"));
      return false;
    }
    // The session runs one script at a time; ignore repeats while busy.
    if (running) {
      setStatus(t("alreadyRunning"));
      return false;
    }
    setRows([]);
    setColumns([]);
    setOutput([]);
    setScriptErrors([]);
    setErrorIndex(-1);
    setScrollTop(0);
    let prepared = text;
    let runOffset = offset;
    if (substitution) {
      const replaced = substituteVariables(text, offset);
      if (!replaced) {
        setStatus(t("substitutionCancelled"));
        return false;
      }
      prepared = replaced.text;
      runOffset = replaced.offset;
    }
    // A single-table select can be edited: run it with the hidden ROWID next to the user's columns.
    const editable = prepareEditable(prepared);
    const executed = editable ? editable.sql : prepared;
    setEditTable(editable ? editable.table : null);
    setRowIds([]);
    setPendingEdits([]);
    // Error offsets only map back to the editor when the text did not change.
    const offsetsValid = prepared === text;
    // Errors from this run are only valid while the text it ran on is intact.
    const runVersion = bufferVersion.current;
    setRunning(true);
    setStatus(t("running"));
    setPanel("results");
    try {
      // A rewritten single-table select adds the ROWID; anything else keeps the user's text.
      let rewriteFailed = false;
      // While the fallback runs there is no ROWID column to strip.
      let usingRowid = editable !== null;
      const onEvent = (event: QueryEvent) => {
          if (event.type === "columns") {
            // Each statement of a script replaces the grid with its own result. The ROWID is not shown:
            // it is kept per row so an edit can address exactly that row.
            setColumns(event.data.filter((column) => column.name !== ROWID_COLUMN));
            setRows([]);
            setRowIds([]);
          } else if (event.type === "batch") {
            const rowidIndex = usingRowid ? event.data.rows[0]?.length - 1 : -1;
            const ids = event.data.rows.map((row) =>
              rowidIndex >= 0 && row[rowidIndex]
                ? cellText(row[rowidIndex])
                : "",
            );
            const values =
              rowidIndex >= 0
                ? event.data.rows.map((row) => row.filter((_, index) => index !== rowidIndex))
                : event.data.rows;
            setRowIds((current) => [...current, ...ids]);
            setRows((current) => [...current, ...values]);
          } else if (event.type === "output") {
            setOutput((current) => [...current, ...event.data]);
            setPanel("output");
          } else if (event.type === "statementerror") {
            if (offsetsValid && bufferVersion.current === runVersion) {
              setScriptErrors((current) => [...current, event.data]);
            }
            setOutput((current) => [
              ...current,
              `${t("error")}: ${event.data.message}`,
            ]);
            setPanel("output");
          } else if (event.type === "done") {
            setRunning(false);
            const label = event.data.rows === 1 ? t("row") : t("rows");
            const errors = event.data.errors
              ? ` · ${event.data.errors} ${t("errors")}`
              : "";
            setStatus(
              `${event.data.rows} ${label} · ${event.data.elapsedMs} ms${errors}`,
            );
          } else {
            rewriteFailed = true;
            setRunning(false);
            setStatus(event.data);
          }
      };
      const exec = (statement: string, historyText?: string) =>
        api.execute(selected, statement, 5000, runOffset, haltOnError, onEvent, historyText);
      try {
        await exec(executed, editable ? prepared : undefined);
        // A view or another object without ROWID cannot carry one: run the user's statement, read-only.
        if (editable && rewriteFailed) {
          usingRowid = false;
          setEditTable(null);
          setRowIds([]);
          setColumns([]);
          setRows([]);
          rewriteFailed = false;
          await exec(prepared);
        }
      } catch (error) {
        if (!editable) throw error;
        // A view or another object without ROWID cannot carry one: run the user's statement, read-only.
        setEditTable(null);
        setRowIds([]);
        setColumns([]);
        setRows([]);
        await exec(prepared);
      }
      return true;
    } catch (error) {
      setRunning(false);
      setStatus(String(error));
      return false;
    }
  };

  const cancel = async () => {
    if (!selected) return;
    try {
      await api.cancel(selected);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const commit = async () => {
    if (!selected) return;
    try {
      await api.commit(selected);
      setTransactionTick((tick) => tick + 1);
      setStatus(t("commit"));
    } catch (error) {
      setStatus(String(error));
    }
  };

  const rollback = async () => {
    if (!selected) return;
    try {
      await api.rollback(selected);
      setTransactionTick((tick) => tick + 1);
      setStatus(t("rollback"));
    } catch (error) {
      setStatus(String(error));
    }
  };

  const showHistory = async () => {
    const at = sideViewRef.current;
    try {
      setHistoryEntries(await api.history());
      // Only switch if the user has not moved to another view while the history was loading.
      if (sideViewRef.current === at) setSideView("history");
    } catch (error) {
      setStatus(String(error));
    }
  };

  const explain = async () => {
    if (!isConnected) {
      setStatus(t("noConnection"));
      return;
    }
    try {
      setPlan(await api.explainPlan(selected, sql));
      setPanel("plan");
    } catch (error) {
      setStatus(String(error));
    }
  };

  const showStats = async () => {
    if (!isConnected) {
      setStatus(t("noConnection"));
      return;
    }
    const at = sideViewRef.current;
    try {
      setStats(await api.sessionStats(selected));
      if (sideViewRef.current === at) setSideView("stats");
    } catch (error) {
      setStatus(String(error));
    }
  };

  const loadObjects = async () => {
    if (!isConnected) {
      setStatus(t("noConnection"));
      return;
    }
    tree.refresh();
  };

  const findObject = async () => {
    if (!isConnected) {
      setStatus(t("noConnection"));
      return;
    }
    const name = window.prompt(t("findObject"));
    if (!name) return;
    try {
      const all = await api.listObjects(selected, schema.trim() || null, null);
      const lower = name.trim().toLowerCase();
      const match =
        all.find((object) => object.name.toLowerCase() === lower) ??
        all.find((object) => object.name.toLowerCase().includes(lower));
      if (!match) {
        setStatus(`${t("findObject")}: ${name}`);
        return;
      }
      await openObject(match);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const qualified = (object: ObjectInfo) =>
    schema.trim() ? `${schema.trim()}.${object.name}` : object.name;

  const openObject = async (object: ObjectInfo) => {
    setSelectedObject(object);
    setObjectColumns([]);
    setConstraints([]);
    setIndexes([]);
    setTriggers([]);
    setGrants([]);
    setDependencies([]);
    setObjectTab("columns");
    setPanel("object");
    if (!["TABLE", "VIEW"].includes(object.objectType)) return;
    try {
      setObjectColumns(
        await api.listColumns(selected, schema.trim() || null, object.name),
      );
    } catch (error) {
      setStatus(String(error));
    }
  };

  const loadDetail = async (tab: ObjectTab) => {
    if (!selectedObject) return;
    setObjectTab(tab);
    const owner = schema.trim() || null;
    try {
      if (tab === "constraints") {
        setConstraints(
          await api.listConstraints(selected, owner, selectedObject.name),
        );
      } else if (tab === "indexes") {
        setIndexes(await api.listIndexes(selected, owner, selectedObject.name));
      } else if (tab === "dependencies") {
        setObjectTab(tab);
        await loadDependencies(selectedObject);
      } else if (tab === "triggers") {
        setTriggers(
          await api.listTriggers(selected, owner, selectedObject.name),
        );
      } else if (tab === "grants") {
        setGrants(await api.listGrants(selected, owner, selectedObject.name));
      }
    } catch (error) {
      setStatus(String(error));
    }
  };

  /** Both directions of ALL_DEPENDENCIES, merged for the Dependencies tab. */
  /** Buffers one cell change; nothing reaches the database until Save. */
  useEffect(() => {
    if (cellView) setEditValue(cellView.value === "NULL" ? "" : cellView.value);
  }, [cellView]);

  const stageEdit = (cell: { column: string; value: string; row: number }, value: string) => {
    const rowId = rowIds[cell.row];
    if (!rowId) {
      setStatus(t("notEditable"));
      return;
    }
    const columnIndex = columns.findIndex((column) => column.name === cell.column);
    const isNull = columnIndex >= 0 && rows[cell.row]?.[columnIndex]?.kind === "null";
    setPendingEdits((current) => [
      ...current.filter((edit) => !(edit.row === cell.row && edit.column === cell.column)),
      {
        row: cell.row,
        rowId,
        column: cell.column,
        oldValue: isNull ? null : cell.value,
        newValue: value === "" ? null : value,
      },
    ]);
    setCellView(null);
    setStatus(`${cell.column}: ${t("pendingChanges")}`);
  };

  /** Applies the buffer; a row changed by somebody else is reported, not overwritten. */
  const saveEdits = async () => {
    if (!editTable || pendingEdits.length === 0) return;
    let saved = 0;
    const conflicts: number[] = [];
    for (const edit of pendingEdits) {
      try {
        const changed = await api.updateCell(
          selected,
          editTable,
          edit.column,
          edit.rowId,
          edit.oldValue,
          edit.newValue,
        );
        if (changed) saved += 1;
        else conflicts.push(edit.row + 1);
      } catch (error) {
        setStatus(String(error));
        return;
      }
    }
    setPendingEdits([]);
    // Refresh first and let the last stream events land: the run writes its own row/time message, so the
    // save result has to be the final word.
    if (conflicts.length === 0) {
      await run(sql);
      await new Promise((resolve) => setTimeout(resolve, 250));
    }
    setStatus(
      conflicts.length
        ? `${saved} ${t("rowsSaved")} · ${t("conflictRows")}: ${conflicts.join(", ")}`
        : `${saved} ${t("rowsSaved")}`,
    );
  };

  const discardEdits = () => {
    setPendingEdits([]);
    setStatus("");
  };

  const applyQbe = () => {
    void run(queryByExample(sql, qbe));
  };

  const loadDependencies = async (object: ObjectInfo) => {
    const owner = schema.trim() || null;
    try {
      const [referenced, dependents] = await Promise.all([
        api.listDependencies(selected, owner, object.name, object.objectType, false),
        api.listDependencies(selected, owner, object.name, object.objectType, true),
      ]);
      setDependencies([
        ...referenced.map((item) => ({ ...item, direction: "referenced" as const })),
        ...dependents.map((item) => ({ ...item, direction: "dependents" as const })),
      ]);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const showDependencies = async (object: ObjectInfo) => {
    await openObject(object);
    setObjectTab("dependencies");
    setPanel("object");
    await loadDependencies(object);
  };

  /** ALTER ... COMPILE; the compiler errors go to the Output tab, like the editor's. */
  const recompileObject = async (object: ObjectInfo) => {
    const owner = schema.trim() || null;
    try {
      await api.compileObject(selected, owner, object.name, object.objectType);
      const errors = await api.listCompileErrors(selected, owner, object.name, object.objectType);
      if (errors.length === 0) {
        setStatus(`${object.name}: ${t("objectValid")}`);
      } else {
        setOutput(errors.map((error) => `ORA ${error.line}:${error.position} ${error.text}`));
        setPanel("output");
        setStatus(`${object.name}: ${errors.length} ${t("compileErrors")}`);
      }
      tree.refresh();
    } catch (error) {
      setStatus(String(error));
    }
  };

  /** Compiles every invalid unit of the schema and reports how many are left. */
  const recompileInvalid = async () => {
    if (!isConnected) {
      setStatus(t("noConnection"));
      return;
    }
    try {
      const [attempted, remaining] = await api.compileInvalidObjects(
        selected,
        schema.trim() || null,
      );
      setInvalidCount(remaining);
      setStatus(
        t("compiledInvalid")
          .replace("{attempted}", String(attempted))
          .replace("{remaining}", String(remaining)),
      );
      tree.refresh();
    } catch (error) {
      setStatus(String(error));
    }
  };

  const toggleTrigger = async (trigger: { name: string; status: string }) => {
    const enabled = !/^ENABLED$/.test(trigger.status);
    try {
      await api.toggleTrigger(selected, schema.trim() || null, trigger.name, enabled);
      setStatus(`${trigger.name}: ${enabled ? t("enable") : t("disable")}`);
      if (objectTab === "triggers") await loadDetail("triggers");
      tree.refresh();
    } catch (error) {
      setStatus(String(error));
    }
  };

  const toggleConstraint = async (constraint: { name: string; status: string }) => {
    if (!selectedObject) return;
    const enabled = !/^ENABLED$/.test(constraint.status);
    try {
      await api.toggleConstraint(
        selected,
        schema.trim() || null,
        selectedObject.name,
        constraint.name,
        enabled,
      );
      setStatus(`${constraint.name}: ${enabled ? t("enable") : t("disable")}`);
      await loadDetail("constraints");
    } catch (error) {
      setStatus(String(error));
    }
  };

  const loadDdlFor = async (object: ObjectInfo) => {
    try {
      setSelectedObject(object);
      const owner = schema.trim() || null;
      const qualifiedName = `${owner ? `${owner}.` : ""}${object.name}`;
      const key = `ddl:${owner ?? ""}.${object.name}.${object.objectType}`;
      let ddl: string;
      if (object.objectType === "TABLE") {
        try {
          ddl = await api.loadDdlMeta(selected, owner, object.name);
        } catch {
          ddl = await api.loadDdl(
            selected,
            owner,
            object.name,
            object.objectType,
          );
        }
      } else {
        ddl = await api.loadDdl(
          selected,
          owner,
          object.name,
          object.objectType,
        );
      }
      const specId = openDoc(
        `${qualifiedName} (${object.objectType})`,
        ddl,
        { key },
      );
      // Packages and types have two parts; the original loads both, here each
      // part gets its own tab so the spec and the body never mix.
      let bodyError = "";
      if (object.objectType === "PACKAGE" || object.objectType === "TYPE") {
        const bodyType = `${object.objectType} BODY`;
        try {
          const body = await api.loadDdl(selected, owner, object.name, bodyType);
          openDoc(`${qualifiedName} (${bodyType})`, body, { key: `${key}:body` });
          setActiveDocId(specId);
        } catch (error) {
          const text = String(error);
          // ORA-31603 is how DBMS_METADATA says "there is no body".
          if (!text.includes("ORA-31603")) bodyError = text;
        }
      }
      setPanel("results");
      setStatus(bodyError || t("loadDdl"));
    } catch (error) {
      setStatus(String(error));
    }
  };

  const queryObject = (object: ObjectInfo) => {
    const text = `select * from ${qualified(object)}`;
    setSql(text);
    void run(text);
  };

  const countObject = (object: ObjectInfo) => {
    const text = `select count(*) from ${qualified(object)}`;
    setSql(text);
    void run(text);
  };

  const transformTable = async (object: ObjectInfo) => {
    if (!selected) {
      setStatus(t("noConnection"));
      return;
    }
    try {
      const script = await api.tableTransformation(
        selected,
        schema.trim() || null,
        object.name,
      );
      setSql(script);
      setStatus(t("transformationHelper"));
    } catch (error) {
      setStatus(String(error));
    }
  };

  const copyName = async (object: ObjectInfo) => {
    try {
      await navigator.clipboard.writeText(object.name);
      setStatus(object.name);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const formatSql = async () => {
    try {
      setSql(
        profileOptions
          ? await api.formatSqlWith(sql, profileOptions)
          : await api.formatSql(sql, true),
      );
    } catch (error) {
      setStatus(String(error));
    }
  };

  const selectProfile = async (name: string) => {
    setProfile(name);
    if (!name) {
      setProfileOptions(null);
      return;
    }
    try {
      setProfileOptions(await api.loadFormatProfile(name));
    } catch (error) {
      setStatus(String(error));
    }
  };

  const saveProfile = async () => {
    const name = window.prompt(t("profileName"), profile || "custom");
    if (!name) return;
    try {
      const options = await api.inferFormatProfile(sql);
      await api.saveFormatProfile(name, options);
      setProfiles(await api.listFormatProfiles());
      setProfile(name);
      setProfileOptions(options);
      setStatus(t("profileSaved"));
    } catch (error) {
      setStatus(String(error));
    }
  };

  const deleteProfile = async () => {
    if (!profile) return;
    if (!window.confirm(`${t("deleteProfile")} ${profile}?`)) return;
    try {
      await api.deleteFormatProfile(profile);
      setProfiles(await api.listFormatProfiles());
      setProfile("");
      setProfileOptions(null);
    } catch (error) {
      setStatus(String(error));
    }
  };
  const dirty = sql !== cleanSql || fileEol !== cleanEol;
  // The buffer and the document being closed are not always the same object for one render after switching
  // tabs, and asking "discard changes?" about the wrong document blocks a close that had nothing to discard.
  const activeDirty =
    activeDoc.sql !== activeDoc.cleanSql || activeDoc.eol !== activeDoc.cleanEol;
  const confirmDiscard = () => !activeDirty || window.confirm(t("discardChanges"));

  const newFile = () => {
    const doc = makeDoc("SQL", "");
    setDocs((current) => [...current, doc]);
    setActiveDocId(doc.id);
    setColumns([]);
    setRows([]);
    setPanel("results");
    setStatus("");
  };

  const openFile = async () => {
    try {
      const path = await api.pickSqlFile();
      if (!path) return;
      const file = await api.readSqlFile(path);
      openDoc(baseName(path), file.text, { path, eol: file.eol });
      setStatus(path);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const saveFile = async (saveAs = false) => {
    const target = activeDoc;
    try {
      let path = target.path;
      if (saveAs || !path) {
        const picked = await api.pickSqlSavePath(target.path || "query.sql");
        if (!picked) return;
        path = picked;
      }
      await api.writeSqlFile(path, target.sql, target.eol);
      patchDoc(target.id, {
        path,
        title: baseName(path),
        cleanSql: target.sql,
        cleanEol: target.eol,
      });
      setStatus(path);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const openFileAtCursor = async () => {
    const path = editorRef.current?.wordAtCursor() ?? "";
    if (!path) {
      setStatus(t("noFile"));
      return;
    }
    try {
      const file = await api.readSqlFile(path);
      openDoc(baseName(path), file.text, { path, eol: file.eol });
      setStatus(path);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const reloadFile = async () => {
    if (!filePath) {
      setStatus(t("noFile"));
      return;
    }
    if (!confirmDiscard()) return;
    const target = activeDoc;
    try {
      const file = await api.readSqlFile(target.path);
      patchDoc(target.id, {
        sql: file.text,
        cleanSql: file.text,
        eol: file.eol,
        cleanEol: file.eol,
      });
      setStatus(target.path);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const reconnect = async () => {
    if (!selected) return;
    try {
      await api.disconnect(selected);
      const version = await api.connect(selected);
      setConnected((current) => ({ ...current, [selected]: version }));
      // A new session has its own transaction: the flag of the old one is no longer valid.
      setTransactionTick((tick) => tick + 1);
      setTables(await api.listTableNames(selected));
      setStatus(`${t("connected")}: ${version}`);
    } catch (error) {
      // The old session was already closed: do not keep claiming it is connected.
      setConnected((current) => {
        const next = { ...current };
        delete next[selected];
        return next;
      });
      setStatus(String(error));
    }
  };

  const toWorkspace = (): Workspace => ({
    version: 1,
    files: docs.map((doc) => ({ name: doc.title, sql: doc.sql })),
    active: Math.max(0, docs.findIndex((doc) => doc.id === activeDoc.id)),
  });

  const restoreWorkspace = (workspace: Workspace) => {
    const files = (workspace.files ?? []).filter(
      (file) => typeof file.sql === "string",
    );
    if (!files.length) return false;
    const fresh = files.map((file) => makeDoc(file.name || "SQL", file.sql));
    setDocs(fresh);
    const index = Math.min(Math.max(workspace.active ?? 0, 0), fresh.length - 1);
    setActiveDocId(fresh[index].id);
    return true;
  };

  const copyWorkspace = async () => {
    try {
      await navigator.clipboard.writeText(JSON.stringify(toWorkspace()));
      setStatus(t("workspaceSave"));
    } catch (error) {
      setStatus(String(error));
    }
  };

  const pasteWorkspace = async () => {
    if (!confirmDiscard()) return;
    try {
      const text = await navigator.clipboard.readText();
      const workspace = JSON.parse(text) as Workspace;
      if (!restoreWorkspace(workspace)) {
        setStatus(t("workspaceLoad"));
        return;
      }
      setStatus(t("workspaceLoad"));
    } catch (error) {
      setStatus(String(error));
    }
  };

  const loadDdlWithDbmsMetadata = async () => {
    if (!selectedObject) {
      setStatus(t("selectObject"));
      return;
    }
    const owner = schema.trim() || null;
    try {
      const ddl = await api.loadDdl(
        selected,
        owner,
        selectedObject.name,
        selectedObject.objectType,
      );
      openDoc(
        `${owner ? `${owner}.` : ""}${selectedObject.name} (DBMS_METADATA)`,
        ddl,
        { key: `ddlmeta:${owner ?? ""}.${selectedObject.name}` },
      );
      setPanel("results");
      setStatus(t("loadDdl"));
    } catch (error) {
      setStatus(String(error));
    }
  };

  const toggleReadOnly = async () => {
    if (!selected) return;
    try {
      const value = await api.toggleReadOnly(selected);
      await refresh();
      setStatus(`${t("readOnly")}: ${value ? "ON" : "OFF"}`);
      if (isConnected) await reconnect();
    } catch (error) {
      setStatus(String(error));
    }
  };

  const enableDbmsOutput = async () => {
    if (!selected) return;
    try {
      await api.enableDbmsOutput(selected);
      setStatus(t("dbmsOutputEnabled"));
    } catch (error) {
      setStatus(String(error));
    }
  };

  const refreshObjectCache = async () => {
    columnCache.current.clear();
    if (!isConnected) return;
    try {
      setTables(await api.listTableNames(selected));
      void loadSuggestions(selected);
      setStatus(t("refresh"));
    } catch (error) {
      setStatus(String(error));
    }
  };
  const checkUpdates = async () => {
    try {
      const info = await api.checkUpdate();
      setUpdateInfo(info);
      setStatus(
        info
          ? `${t("updateAvailable")}: ${info.version}`
          : t("updateNone"),
      );
    } catch (error) {
      setStatus(String(error));
    }
  };

  const installUpdate = async () => {
    setInstalling(true);
    try {
      await api.installUpdate((progress) => {
        setStatus(
          `${t("updateProgress")}: ${progress.downloaded}${
            progress.total ? ` / ${progress.total}` : ""
          }`,
        );
      });
      setStatus(t("updateInstalled"));
      setUpdateInfo(null);
    } catch (error) {
      setStatus(String(error));
    } finally {
      setInstalling(false);
    }
  };
  const saveWorkspace = async () => {
    try {
      await api.saveWorkspace(toWorkspace());
      setStatus(t("workspaceSave"));
    } catch (error) {
      setStatus(String(error));
    }
  };

  const saveWorkspaceAs = async () => {
    try {
      const path = await api.pickWorkspaceSavePath("workspace.json");
      if (!path) return;
      await api.saveWorkspaceTo(path, toWorkspace());
      setStatus(path);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const createTemplate = async () => {
    const name = window.prompt(t("templateName"));
    if (!name) return;
    const body = editorRef.current?.selectedText() || sql;
    try {
      await api.saveTemplate(name, body);
      setStatus(`${t("templateSaved")}: ${name}`);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const openSnippets = async () => {
    try {
      setSnippetNames(await api.listTemplates());
      setSnippetsOpen(true);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const insertSnippet = async (name: string) => {
    try {
      const template = await api.loadTemplate(name);
      if (!template) {
        setStatus(`${t("templateMissing")}: ${name}`);
        return;
      }
      editorRef.current?.insertText(template.body);
      setSnippetsOpen(false);
      setStatus(`${t("templateExpanded")}: ${name}`);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const navigateError = (direction: 1 | -1) => {
    if (scriptErrors.length === 0) {
      setStatus(t("noErrors"));
      return;
    }
    const sorted = [...scriptErrors].sort((a, b) => a.offset - b.offset);
    const index =
      direction === 1
        ? (errorIndex + 1) % sorted.length
        : (errorIndex - 1 + sorted.length) % sorted.length;
    setErrorIndex(index);
    editorRef.current?.goToOffset(sorted[index].offset);
    setStatus(`${t("error")} ${index + 1}/${sorted.length}`);
  };

  const extractSchemaDdl = async () => {
    if (!selected) return;
    const answer = window.prompt(t("maxObjects"), "50");
    if (answer === null) return;
    const limit = Number(answer);
    if (!Number.isFinite(limit) || limit <= 0) {
      setStatus(t("invalidNumber"));
      return;
    }
    try {
      const path = await api.pickSqlSavePath("schema_ddl.sql");
      if (!path) return;
      const count = await api.extractSchemaDdl(
        selected,
        schema.trim() || null,
        objectType.trim() || null,
        Math.floor(limit),
        path,
      );
      setStatus(`${t("extracted")}: ${count} → ${path}`);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const openNewExtended = async () => {
    try {
      setSnippetNames(await api.listTemplates());
      setNewExtendedOpen(true);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const createFromTemplate = async (name: string | null) => {
    setNewExtendedOpen(false);
    let text = "";
    if (name) {
      try {
        const template = await api.loadTemplate(name);
        if (!template) {
          setStatus(`${t("templateMissing")}: ${name}`);
          return;
        }
        text = template.body;
        setStatus(`${t("templateExpanded")}: ${name}`);
      } catch (error) {
        setStatus(String(error));
        return;
      }
    }
    const doc = makeDoc("SQL", text);
    setDocs((current) => [...current, doc]);
    setActiveDocId(doc.id);
    setColumns([]);
    setRows([]);
    setOutput([]);
    setScriptErrors([]);
    setErrorIndex(-1);
    setPanel("results");
  };

  const executeCurrentAndStep = async () => {
    try {
      const ranges = await api.statementRanges(sql);
      if (ranges.length === 0) {
        setStatus(t("noStatement"));
        return;
      }
      const cursor = editorRef.current?.cursorOffset() ?? 0;
      const index = ranges.findIndex(
        (range) => cursor >= range.start && cursor <= range.end,
      );
      const current =
        index >= 0
          ? ranges[index]
          : (ranges.filter((range) => range.start <= cursor).pop() ??
            ranges[0]);
      const next = ranges[ranges.indexOf(current) + 1];
      // Only advance when the execution was accepted (connected and idle).
      const started = await run(sql, current.start);
      if (!started) return;
      if (next) editorRef.current?.goToOffset(next.start);
      setStatus(`${t("step")} ${ranges.indexOf(current) + 1}/${ranges.length}`);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const stepStatement = async (direction: 1 | -1) => {
    try {
      const ranges = await api.statementRanges(sql);
      const cursor = editorRef.current?.cursorOffset() ?? 0;
      const target =
        direction === 1
          ? ranges.find((range) => range.start > cursor)
          : [...ranges].reverse().find((range) => range.start < cursor);
      if (!target) {
        setStatus(t("noStatement"));
        return;
      }
      editorRef.current?.goToOffset(target.start);
      await showHistory();
    } catch (error) {
      setStatus(String(error));
    }
  };

  const buildOutline = () => {
    const entries: { line: number; label: string }[] = [];
    sql.split("\n").forEach((line, index) => {
      const trimmed = line.trim();
      if (!trimmed) return;
      const create =
        /^create\s+(?:or\s+replace\s+)?(?:editionable\s+)?(table|view|materialized\s+view|procedure|function|package|trigger|type|sequence)\s+([\w$#."]+)/i.exec(
          trimmed,
        );
      if (create) {
        entries.push({
          line: index + 1,
          label: `${create[1].toUpperCase()} ${create[2]}`,
        });
        return;
      }
      const first = /^([a-z]+)/i.exec(trimmed)?.[1]?.toUpperCase() ?? "";
      if (
        [
          "SELECT",
          "INSERT",
          "UPDATE",
          "DELETE",
          "MERGE",
          "WITH",
          "BEGIN",
          "DECLARE",
        ].includes(first)
      ) {
        entries.push({ line: index + 1, label: first });
      }
    });
    setOutline(entries);
    setOutlineOpen(true);
  };

  const expandTemplate = async () => {
    const name = window.prompt(t("templateName"));
    if (!name) return;
    try {
      const template = await api.loadTemplate(name);
      if (!template) {
        setStatus(`${t("templateMissing")}: ${name}`);
        return;
      }
      editorRef.current?.insertText(template.body);
      setStatus(`${t("templateExpanded")}: ${name}`);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const copyFileLocation = async () => {
    if (!filePath) {
      setStatus(t("noFile"));
      return;
    }
    try {
      await navigator.clipboard.writeText(filePath);
      setStatus(filePath);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const loadWorkspace = async () => {
    if (!confirmDiscard()) return;
    try {
      const workspace: Workspace = await api.loadWorkspace();
      restoreWorkspace(workspace);
      setStatus(t("workspaceLoad"));
    } catch (error) {
      setStatus(String(error));
    }
  };

  const importLegacy = async () => {
    const path = window.prompt(t("legacyPath"));
    if (!path) return;
    try {
      const count = await api.migrateLegacy(path);
      await refresh();
      setStatus(`${count} ${t("connections")}`);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const executeCommand = (name: string) => {
    setPaletteOpen(false);
    // `File.Print`/`PrintPreview`/`PrintSetup`: the print dialog carries the
    // preview and the printer choice (WebView2 exposes both), so all three print
    // the script through the print stylesheet. CodeMirror only renders the
    // viewport, hence the hidden <pre> with the whole document.
    const printScript = () => {
      const node = printScriptRef.current;
      if (!node) return;
      node.textContent = sql;
      window.addEventListener(
        "afterprint",
        () => {
          node.textContent = "";
        },
        { once: true },
      );
      window.print();
    };
    const slot = /^Edit\.(SetRandomBookmark|GetRandomBookmark)\.(\d)$/.exec(name);
    if (slot) {
      const index = Number(slot[2]);
      if (slot[1] === "SetRandomBookmark") {
        editorRef.current?.setSlotBookmark(index);
      } else {
        editorRef.current?.goToSlotBookmark(index);
      }
      return;
    }
    switch (name) {
      case "File.PageSetup":
        // The original maps this to an id with no handler, and a webview has no
        // separate page-setup API (Codex review, 2026-11-17): inform instead of
        // adding settings that the print dialog may override.
        setStatus(t("pageSetupInfo"));
        break;
      case "Tools.TableTransformationHelper":
        if (selectedObject) void transformTable(selectedObject);
        else setStatus(t("selectObject"));
        break;
      case "File.Print":
      case "File.PrintPreview":
      case "File.PrintSetup":
        printScript();
        break;
      case "File.New":
        newFile();
        break;
      case "Session.Reconnect":
        void reconnect();
        break;
      case "Session.ToggleReadOnly":
        void toggleReadOnly();
        break;
      case "Script.QuickCount":
        if (selectedObject) countObject(selectedObject);
        else setStatus(t("selectObject"));
        break;
      case "View.Properties":
        setPanel("object");
        break;
      case "File.Open":
        void openFile();
        break;
      case "File.Save":
        void saveFile();
        break;
      case "File.SaveAs":
        void saveFile(true);
        break;
      case "File.SaveAll":
        void saveFile();
        break;
      case "File.Reload":
        void reloadFile();
        break;
      case "Edit.FileFormat_Winows":
        setFileEol("\r\n");
        setStatus(`${t("lineEnding")}: CRLF`);
        break;
      case "Edit.FileFormat_Unix":
        setFileEol("\n");
        setStatus(`${t("lineEnding")}: LF`);
        break;
      case "Edit.FileFormat_Mac":
        setFileEol("\r");
        setStatus(`${t("lineEnding")}: CR`);
        break;
      case "File.Close":
        if (!confirmDiscard()) break;
        closeDoc(activeDoc.id);
        break;
      case "File.CloseAll":
        if (!confirmDiscard()) break;
        resetDocs();
        break;
      case "Session.RefreshObjectCache":
        void refreshObjectCache();
        break;
      case "Session.Connect":
        void connect();
        break;
      case "Session.Disconnect":
        void disconnect();
        break;
      case "Session.Commit":
        void commit();
        break;
      case "Session.Rollback":
        void rollback();
        break;
      case "Session.Cancel":
        void cancel();
        break;
      case "Session.EnableSessionStatistics":
        void showStats();
        break;
      case "Session.EnableDbmsOutput":
        void enableDbmsOutput();
        break;
      case "Script.Execute":
        void run();
        break;
      case "Script.ExecuteCurrent":
        void run(sql, editorRef.current?.cursorOffset() ?? null);
        break;
      case "Script.ExplainPlan":
        void explain();
        break;
      case "Script.ExecuteInSQLPlus": {
        if (!selected) {
          setStatus(t("noConnection"));
          break;
        }
        // The original runs the selection when there is one, else the document.
        const text = editorRef.current?.selectedText() || sql;
        void api
          .executeInSqlPlus(selected, text)
          .then((path) => setStatus(`${t("executingSqlPlus")}: ${path}`))
          .catch((error) => setStatus(String(error)));
        break;
      }
      case "Script.LoadDDL":
        if (selectedObject) void loadDdlFor(selectedObject);
        else setStatus(t("selectObject"));
        break;
      case "View.ObjectList":
      case "View.ObjectTree":
        setSideView("objects");
        void loadObjects();
        break;
      case "View.History":
        void showHistory();
        break;
      case "Edit.Undo":
        editorRef.current?.undo();
        break;
      case "Edit.Redo":
        editorRef.current?.redo();
        break;
      case "Edit.SelectAll":
        editorRef.current?.selectAll();
        break;
      case "Edit.Copy":
        void editorRef.current?.copy();
        break;
      case "Edit.Cut":
        void editorRef.current?.cut();
        break;
      case "Edit.Paste":
        void editorRef.current?.paste();
        break;
      case "Edit.Uppercase":
        editorRef.current?.upper();
        break;
      case "Edit.Lowercase":
        editorRef.current?.lower();
        break;
      case "Edit.CommentLine":
        editorRef.current?.comment();
        break;
      case "Edit.UncommentLine":
        editorRef.current?.uncomment();
        break;
      case "Edit.InvertCase":
        editorRef.current?.invertCase();
        break;
      case "Edit.Capitalize":
        editorRef.current?.capitalize();
        break;
      case "Edit.SelectLine":
        editorRef.current?.selectLine();
        break;
      case "Edit.DeleteLine":
        editorRef.current?.deleteLine();
        break;
      case "Edit.DupLineOrSelection":
        editorRef.current?.duplicateLine();
        break;
      case "Edit.JoinLines":
        editorRef.current?.joinLines();
        break;
      case "Edit.IndentSelection":
        editorRef.current?.indent();
        break;
      case "Edit.UndentSelection":
        editorRef.current?.outdent();
        break;
      case "Edit.Find":
      case "Edit.Replace":
      case "Edit.FindInFile":
        editorRef.current?.find();
        break;
      case "Edit.OpenFileUnderCursor":
        void openFileAtCursor();
        break;
      case "Edit.FindNext":
        editorRef.current?.findNext();
        break;
      case "Edit.FindPrevious":
        editorRef.current?.findPrevious();
        break;
      case "Edit.ExpandAllFolds":
        editorRef.current?.unfoldAll();
        break;
      case "Edit.CollapseAllFolds":
        editorRef.current?.foldAll();
        break;
      case "Edit.TrimTrailingSpaces":
        editorRef.current?.trimTrailing();
        break;
      case "Edit.RemoveBlankLines":
        editorRef.current?.removeBlankLines();
        break;
      case "Edit.Sort":
        editorRef.current?.sortLines();
        break;
      case "Edit.ToggleBookmark":
        editorRef.current?.toggleBookmark();
        break;
      case "Edit.NextBookmark":
        editorRef.current?.nextBookmark();
        break;
      case "Edit.PreviousBookmark":
        editorRef.current?.previousBookmark();
        break;
      case "Edit.RemoveallBookmarks":
        editorRef.current?.clearBookmarks();
        break;
      case "Edit.CopyBookmarkedLines":
        void editorRef.current?.copyBookmarkedLines();
        break;
      case "Edit.CutBookmarkedLines":
        void editorRef.current?.cutBookmarkedLines();
        break;
      case "Edit.DeleteBookmarkedLines":
        editorRef.current?.deleteBookmarkedLines();
        break;
      case "Edit.TabifySelectionAll":
        editorRef.current?.tabifyAll();
        break;
      case "Edit.TabifySelectionLeading":
        editorRef.current?.tabifyLeading();
        break;
      case "Edit.UntabifySelection":
        editorRef.current?.untabify();
        break;
      case "Edit.TrimLeadingSpaces":
        editorRef.current?.trimLeading();
        break;
      case "Edit.TrimExcessiveSpaces":
        editorRef.current?.trimExcessive();
        break;
      case "Edit.RemoveExcessiveBlankLines":
        editorRef.current?.removeExcessiveBlankLines();
        break;
      case "Edit.SplitLine":
        editorRef.current?.splitLine();
        break;
      case "Edit.NormalizeKeyword":
        editorRef.current?.normalizeKeyword();
        break;
      case "Edit.GoTo":
        editorRef.current?.goToLine();
        break;
      case "Edit.DatetimeStamp":
        editorRef.current?.timestamp();
        break;
      case "Script.ExecuteFromCursor":
        void run(editorRef.current?.textFromCursor() ?? sql);
        break;
      case "Script.ExecuteCurrentAlt":
        void run();
        break;
      case "Script.QuickQuery":
        if (selectedObject) queryObject(selectedObject);
        else setStatus(t("selectObject"));
        break;
      case "Script.FindObject":
        void findObject();
        break;
      case "Script.LoadDDLWithDbmsMetadata":
        void loadDdlWithDbmsMetadata();
        break;
      case "View.StatusBar":
        setShowStatus((visible) => !visible);
        break;
      case "Edit.FormatSQL":
        void formatSql();
        break;
      case "Edit.ToggleAllFolds":
        editorRef.current?.toggleAllFolds();
        break;
      case "Edit.CollapseCurrentRegion":
        editorRef.current?.collapseRegion();
        break;
      case "Edit.ExpandCurrentRegion":
        editorRef.current?.expandRegion();
        break;
      case "Edit.ToggleCurrentRegion":
        editorRef.current?.toggleRegion();
        break;
      case "Edit.ExpandCurrentSubtree":
        editorRef.current?.expandSubtree();
        break;
      case "Edit.Autocomplete":
        editorRef.current?.autocomplete();
        break;
      case "Edit.ToggleFoldingEnabled": {
        const enabled = editorRef.current?.toggleFolding();
        setStatus(enabled ? t("foldingOn") : t("foldingOff"));
        break;
      }
      case "Grid.ShowPopupViewer":
        if (activeCell) setCellView(activeCell);
        else setStatus(t("selectCell"));
        break;
      case "Help.Help":
      case "Help.ContextHelp":
        setHelpOpen(true);
        break;
      case "View.Toolbar":
        setShowToolbar(!showToolbar);
        break;
      case "View.NextPane":
      case "Window.NextPane":
        focusPane(activePane() + 1);
        break;
      case "View.PrevPane":
      case "Window.PreviousPane":
        focusPane(activePane() - 1);
        break;
      case "View.ExplainPlan":
        setPanel("plan");
        break;
      case "View.ObjectTreeSetFocus":
        focusObjectsFilter();
        break;
      case "Edit.FormatSQLwithSettings":
        if (profileDetailsRef.current) profileDetailsRef.current.open = true;
        profileSelectRef.current?.focus();
        setStatus(t("formatterProfiles"));
        break;
      case "Tools.SessionDdlGridSettings":
        setSettingsSection("app");
        setSettingsOpen(true);
        break;
      case "Workspace.OpenQuick":
        void loadWorkspace();
        break;
      case "Edit.SelectWord":
        editorRef.current?.selectWord();
        break;
      case "Edit.CopyAppend":
        void editorRef.current?.copyAppend();
        break;
      case "Edit.CutAppend":
        void editorRef.current?.cutAppend();
        break;
      case "Edit.Delete":
        editorRef.current?.deleteForward();
        break;
      case "Edit.DeleteWordBack":
        editorRef.current?.deleteWordBack();
        break;
      case "Edit.DeletetoEndOfWord":
        editorRef.current?.deleteToEndOfWord();
        break;
      case "Edit.ScrollUp":
        editorRef.current?.scrollUp();
        break;
      case "Edit.ScrollDown":
        editorRef.current?.scrollDown();
        break;
      case "Edit.ScrollToCenter":
        editorRef.current?.scrollToCenter();
        break;
      case "Edit.FindSelectedNext":
        editorRef.current?.findSelected(true);
        break;
      case "Edit.FindSelectedPrevious":
        editorRef.current?.findSelected(false);
        break;
      case "Edit.LineNumbers":
        editorRef.current?.toggleLineNumbers();
        break;
      case "Edit.FindMatch":
        editorRef.current?.matchBracket();
        break;
      case "Edit.FindMatchSelect":
        editorRef.current?.selectToMatch();
        break;
      case "Edit.CollapseToLevel1":
        editorRef.current?.foldToLevel(1);
        break;
      case "Edit.CollapseToLevel2":
        editorRef.current?.foldToLevel(2);
        break;
      case "Edit.CollapseToLevel3":
        editorRef.current?.foldToLevel(3);
        break;
      case "Edit.SyntaxGutter":
        editorRef.current?.toggleSyntaxGutter();
        break;
      case "Edit.Ruler":
        editorRef.current?.toggleRuler();
        break;
      case "Edit.ColumnMarkers":
        editorRef.current?.toggleColumnMarkers();
        break;
      case "Edit.VisibleSpaces":
        editorRef.current?.toggleVisibleSpaces();
        break;
      case "Edit.ColumnLeftJustify":
        editorRef.current?.justifyColumn("left");
        break;
      case "Edit.ColumnRightJustify":
        editorRef.current?.justifyColumn("right");
        break;
      case "Edit.ColumnCenterJustify":
        editorRef.current?.justifyColumn("center");
        break;
      case "Edit.ColumnInsert":
        editorRef.current?.insertColumn();
        break;
      case "Edit.ColumnInsertNumber":
        editorRef.current?.insertColumnNumbers();
        break;
      case "Edit.CreateTemplate":
        void createTemplate();
        break;
      case "Edit.ExpandTemplate":
        void expandTemplate();
        break;
      case "Edit.FileSettings":
        setSettingsSection("file");
        setSettingsOpen(true);
        break;
      case "Edit.PermanentSettings":
        setSettingsSection("app");
        setSettingsOpen(true);
        break;
      case "Script.HaltOnError": {
        const next = !haltOnError;
        setHaltOnError(next);
        setStatus(`${t("haltOnError")}: ${next ? t("on") : t("off")}`);
        break;
      }
      case "Script.SubstitutionVariables": {
        const next = !substitution;
        setSubstitution(next);
        setStatus(`${t("substitution")}: ${next ? t("on") : t("off")}`);
        break;
      }
      case "File.Exit":
        if (!confirmDiscard()) break;
        void api.exitApp();
        break;
      case "File.ShowFileLocation":
        if (!filePath) {
          setStatus(t("noFile"));
          break;
        }
        void api
          .revealFileLocation(filePath)
          .catch((error) => setStatus(String(error)));
        break;
      case "Help.SqlHelp":
        void api.openSqlHelp().catch((error) => setStatus(String(error)));
        break;
      case "View.Snippets":
        void openSnippets();
        break;
      case "View.CodeOutline":
        buildOutline();
        break;
      case "View.OpenFiles":
        setOpenFilesOpen(true);
        break;
      case "View.FilePanel":
        setFilePanelOpen(true);
        break;
      case "Tools.ExtractSchemaDDL":
        void extractSchemaDdl();
        break;
      case "Window.ActivateSplitter": {
        const next = !splitterActive;
        setSplitterActive(next);
        if (next) splitterRef.current?.focus();
        setStatus(`${t("splitter")}: ${next ? t("on") : t("off")}`);
        break;
      }
      case "Window.SplitterDown":
        nudgeSplit(5);
        break;
      case "Window.SplitterUp":
        nudgeSplit(-5);
        break;
      case "View.FindInFiles":
        setFindQuery("");
        setFindInFilesOpen(true);
        break;
      case "File.NewExtended":
        void openNewExtended();
        break;
      case "Script.ExecuteCurrentAndStep":
        void executeCurrentAndStep();
        break;
      case "Script.StepForwardAndGetHistory":
        void stepStatement(1);
        break;
      case "Script.GetHistoryAndStepBack":
        void stepStatement(-1);
        break;
      case "Script.NextError":
        navigateError(1);
        break;
      case "Script.PreviousError":
        navigateError(-1);
        break;
      case "Edit.ColumnSelection":
      case "Edit.StreamSelection":
      case "Edit.ToggleSelectionMode": {
        const mode =
          name === "Edit.ColumnSelection"
            ? "column"
            : name === "Edit.StreamSelection"
              ? "stream"
              : "toggle";
        const rectangular = editorRef.current?.setSelectionMode(mode);
        setStatus(
          `${t("selectionMode")}: ${
            rectangular ? t("columnSelection") : t("streamSelection")
          }`,
        );
        break;
      }
      case "Edit.IndentGuide":
        editorRef.current?.toggleIndentGuides();
        break;
      case "Workspace.SaveQuick":
        void saveWorkspace();
        break;
      case "Workspace.SaveAs":
        void saveWorkspaceAs();
        break;
      case "File.CopyFileLocation":
        void copyFileLocation();
        break;
      case "Help.AboutOpenEditor":
        window.alert(`SQLTools 0.1.0\n${t("app.subtitle")}`);
        break;
      case "Workspace.OpenAutosaved":
        void loadWorkspace();
        break;
      case "Workspace.Copy":
        void copyWorkspace();
        break;
      case "Workspace.Paste":
        void pasteWorkspace();
        break;
      case "Workspace.OpenClose":
        newFile();
        break;
      case "Workspace.Save":
        void saveWorkspace();
        break;
      case "Workspace.Open":
        void loadWorkspace();
        break;
      default:
        setStatus(`${t("notImplemented")}: ${name || "internal"}`);
    }
    // The palette unmounts with its input: if the command itself did not take the focus (a dialog, a
    // field), it goes back to the editor instead of being lost on <body>.
    window.setTimeout(() => {
      if (!document.activeElement || document.activeElement === document.body) {
        editorRef.current?.focus();
      }
    }, 0);
  };

  // The shortcut binder reads the latest dispatcher without re-subscribing.
  useEffect(() => {
    commandRunner.current = executeCommand;
  });

  const filteredCommands = commands
    .filter((command) => command.name)
    .filter((command) => {
      const query = paletteQuery.toLowerCase();
      return (
        !query ||
        command.name.toLowerCase().includes(query) ||
        command.id.toLowerCase().includes(query) ||
        command.keys.some((key) => key.toLowerCase().includes(query))
      );
    })
    .slice(0, 50);

  const onPaletteKey = (event: ReactKeyboardEvent<HTMLInputElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setPaletteIndex((index) =>
        filteredCommands.length ? (index + 1) % filteredCommands.length : 0,
      );
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setPaletteIndex((index) =>
        filteredCommands.length
          ? (index - 1 + filteredCommands.length) % filteredCommands.length
          : 0,
      );
    } else if (event.key === "Enter") {
      event.preventDefault();
      const command = filteredCommands[paletteIndex];
      if (command) executeCommand(command.name);
    } else if (event.key === "Escape") {
      event.preventDefault();
      setPaletteOpen(false);
      editorRef.current?.focus();
    }
  };
  const exportCsv = async () => {
    if (isConnected) {
      try {
        const path = await api.exportCsvFile(selected, sql, 0);
        setStatus(path);
        return;
      } catch (error) {
        setStatus(String(error));
      }
    }
    if (!columns.length) return;
    const header = columns.map((column) => column.name).join(",");
    const lines = rows.map((row) => row.map(csvCell).join(","));
    const blob = new Blob([[header, ...lines].join("\n")], {
      type: "text/csv;charset=utf-8",
    });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = "sqltools.csv";
    link.click();
    URL.revokeObjectURL(url);
  };


  const findMatches = findQuery
    ? sql
        .split("\n")
        .map((line, index) => ({ line: index + 1, text: line }))
        .filter((entry) =>
          entry.text.toLowerCase().includes(findQuery.toLowerCase()),
        )
        .map((entry) => ({
          line: entry.line,
          text: entry.text.trim().slice(0, 120),
        }))
    : [];

  const connectionLabel = connections.find((config) => config.id === selected)?.label ?? "";
  // A DDL tab is described by its own key (`ddl:owner.name.TYPE`), not by the current connection and schema:
  // after switching either, the tab still holds the text fetched from the old source.
  const ddlKey = /^ddl:([^.]*)\.(.*)\.([^.]+)$/.exec(activeDoc.key ?? "");
  const breadcrumb: string[] = ddlKey
    ? [ddlKey[1], ddlKey[3], ddlKey[2]].filter(Boolean)
    : [
        isConnected ? connectionLabel || draft.user : t("noConnection"),
        ...(isConnected ? [schema.trim() || draft.user.toUpperCase()] : []),
        activeDoc.path ? (activeDoc.path.split(/[\\/]/).pop() ?? activeDoc.path) : t("untitledScript"),
      ];

  return (
    <div className="app">
      <pre className="print-script" ref={printScriptRef} aria-hidden="true" />
      <header className="app-header">
      <TitleBar
        isConnected={isConnected}
        connLabel={`${draft.user}${draft.alias ? `@${draft.alias}` : ""}`}
        environment={isConnected ? environment : null}
        connTitle={isConnected ? (connected[selected] ?? "") : t("noConnection")}
        theme={themeMode}
        onTheme={() =>
          setThemeMode(
            themeMode === "light"
              ? "dark"
              : themeMode === "dark"
                ? "system"
                : "light",
          )
        }
        language={language}
        onLanguage={setLanguage}
        palette={{
          open: paletteOpen,
          query: paletteQuery,
          index: paletteIndex,
          commands: filteredCommands,
          formatKeys,
          onOpen: () => setPaletteOpen(true),
          onClose: () => setPaletteOpen(false),
          onQuery: (query) => {
            setPaletteQuery(query);
            setPaletteIndex(0);
          },
          onKeyDown: onPaletteKey,
          onRun: executeCommand,
        }}
        onChip={openConnections}
      />
      {showToolbar && (
        <ActionStrip
          isConnected={isConnected}
          hasSelection={Boolean(selected)}
          running={running}
          hasSql={Boolean(sql.trim())}
          readOnly={draft.readOnly}
          transactionPending={transactionPending}
          onReadOnly={(value) => update("readOnly", value)}
          onRun={() => void run()}
          onCancel={() => void cancel()}
          onFormat={() => void formatSql()}
          onExplain={() => void explain()}
          onCommit={() => void commit()}
          onRollback={() => void rollback()}
          onConnectToggle={() => void (isConnected ? disconnect() : connect())}
          moreAlert={Boolean(updateInfo)}
          more={[
            { id: "ws-save", label: t("workspaceSave"), onClick: () => void saveWorkspace() },
            { id: "ws-load", label: t("workspaceLoad"), onClick: () => void loadWorkspace() },
            { id: "cache", label: t("refresh"), onClick: () => void refreshObjectCache(), disabled: !isConnected },
            ...(updateInfo
              ? [{ id: "install", label: `${t("installUpdate")}: ${updateInfo.version}`, onClick: () => void installUpdate(), disabled: installing }]
              : []),
            { id: "check", label: t("checkUpdates"), onClick: () => void checkUpdates() },
          ]}
          breadcrumb={breadcrumb}
        />
      )}
      {!showToolbar && (
        <button
          className="tb toolbar-restore"
          title={t("toolbar")}
          onClick={() => setShowToolbar(true)}
        >
          ▾
        </button>
      )}

      </header>
      <div className="body">
        <ActivityBar
          view={sideView}
          onView={handleSideView}
          onShortcuts={() => setHelpOpen(true)}
          onSettings={() => {
            setSettingsSection("app");
            setSettingsOpen(true);
          }}
        />
        <aside className="objects-panel">
          {sideView === "objects" && (
            <>
          <div className="panel-title">
            {t("objects")}
            {isConnected && <EnvBadge environment={environment} />}
          </div>
          <div className="object-filter">
            <input
              value={schema}
              placeholder={t("schema")}
              aria-label={t("schema")}
              title={t("schema")}
              onChange={(event) => setSchema(event.target.value)}
            />
            <input
              ref={objectsFilterRef}
              value={tree.filter}
              placeholder={t("filter")}
              aria-label={t("filter")}
              title={t("filter")}
              onChange={(event) => tree.setFilter(event.target.value)}
            />
            <button
              className="tb"
              title={t("loadObjects")}
              aria-label={t("loadObjects")}
              onClick={() => void loadObjects()}
              disabled={!isConnected}
            >
              <Icon name="refresh" />
            </button>
            <button
              className="tb"
              title={`${t("recompileInvalid")} (${invalidCount})`}
              aria-label={t("recompileInvalid")}
              onClick={() => void recompileInvalid()}
              disabled={!isConnected || invalidCount === 0}
            >
              <Icon name="warning" />
            </button>
          </div>
          {isConnected ? (
            <>
              {tree.filter.trim() !== "" && (
                <div className="tree-search">
                  {tree.loadingAll ? (
                    <button className="tb text" onClick={tree.cancelLoadAll}>
                      {t("cancel")}
                    </button>
                  ) : (
                    <button className="tb text" onClick={() => void tree.loadAll()}>
                      {t("searchAll")}
                    </button>
                  )}
                </div>
              )}
              <ObjectTree
                rows={tree.rows}
                label={t("objects")}
                onToggle={tree.toggle}
                onExpand={tree.expand}
                onRetry={(type) => void tree.retry(type)}
                onOpen={(object) => void openObject(object)}
                onContext={(object, x, y) => setMenu({ object, x, y })}
                onGroupFocus={setObjectType}
              />
            </>
          ) : (
            <p className="side-empty">{t("connectToExplore")}</p>
          )}
            </>
          )}

          {sideView === "connections" && (
            <ConnectionsView
              connections={connections}
              selected={selected}
              connected={connected}
              draft={draft}
              password={password}
              update={update}
              setPassword={setPassword}
              onSelect={selectConnection}
              onNew={newConnection}
              onSave={() => void save()}
              onConnect={() => void connect()}
              onRemove={() => void remove()}
              onImportLegacy={() => void importLegacy()}
            />
          )}

          {sideView === "history" && (
            <HistoryView entries={historyEntries} onPick={setSql} />
          )}

          {sideView === "stats" && <StatsView stats={stats} />}

          <details className="side-section" ref={profileDetailsRef}>
            <summary>{t("formatterProfiles")}</summary>
            <div className="section-body">
              <label className="field-row">
                <span>{t("profile")}</span>
                <select
                  ref={profileSelectRef}
                  value={profile}
                  onChange={(event) => void selectProfile(event.target.value)}
                >
                  <option value="">{t("defaultProfile")}</option>
                  {profiles.map((name) => (
                    <option key={name} value={name}>
                      {name}
                    </option>
                  ))}
                </select>
              </label>
              <div className="row">
                <button className="plain" onClick={() => void saveProfile()}>
                  {t("saveProfile")}
                </button>
                <button
                  className="plain"
                  onClick={() => void deleteProfile()}
                  disabled={!profile}
                >
                  {t("deleteProfile")}
                </button>
              </div>
            </div>
          </details>
        </aside>

        <main className="workspace" ref={workspaceRef}>
          <div
            className="editor-shell"
            style={{ flex: `0 0 ${splitPercent}%` }}
          >
            <EditorTabs
              docs={docs.map((doc) => ({
                id: doc.id,
                title: doc.title,
                path: doc.path,
                dirty: doc.sql !== doc.cleanSql || doc.eol !== doc.cleanEol,
              }))}
              activeId={activeDoc.id}
              onSelect={(id) => {
                setActiveDocId(id);
                editorRef.current?.focus();
              }}
              onClose={closeDoc}
            />
            <Editor
              ref={editorRef}
              initialValue={sql}
              tables={tables}
              objects={objectSuggestions}
              loadColumns={loadColumnsFor}
              onChange={setSql}
              onExecute={(text, offset) => void run(text, offset)}
              onCursor={setCursor}
              label={t("editor")}
              theme={darkTheme ? "dark" : "light"}
            />
          </div>

          <div
            className={splitterActive ? "splitter active" : "splitter"}
            role="separator"
            aria-orientation="horizontal"
            aria-label={t("splitter")}
            aria-valuenow={Math.round(splitPercent)}
            aria-valuemin={20}
            aria-valuemax={85}
            tabIndex={0}
            title={t("splitter")}
            ref={splitterRef}
            onPointerDown={(event) =>
              event.currentTarget.setPointerCapture(event.pointerId)
            }
            onPointerMove={(event) => {
              if (!event.currentTarget.hasPointerCapture(event.pointerId)) {
                return;
              }
              const rect = workspaceRef.current?.getBoundingClientRect();
              if (!rect || rect.height === 0) return;
              setSplitPercentValue(
                ((event.clientY - rect.top) / rect.height) * 100,
              );
            }}
            onKeyDown={(event) => {
              if (event.key === "ArrowUp") {
                event.preventDefault();
                nudgeSplit(-5);
              } else if (event.key === "ArrowDown") {
                event.preventDefault();
                nudgeSplit(5);
              }
            }}
          />

          <ResultsPanel
            panel={panel}
            setPanel={setPanel}
            columns={columns}
            rows={rows}
            scrollTop={scrollTop}
            setScrollTop={setScrollTop}
            setRows={setRows}
            setColumns={setColumns}
            running={running}
            isConnected={isConnected}
            dataPanelRef={dataPanelRef}
            exportCsv={exportCsv}
            setStatus={setStatus}
              setActiveCell={setActiveCell}
            setCellView={setCellView}
                    plan={plan}
            output={output}
            selectedObject={selectedObject}
              objectTab={objectTab}
            loadDetail={loadDetail}
            loadDdlFor={loadDdlFor}
            objectColumns={objectColumns}
            constraints={constraints}
            indexes={indexes}
            triggers={triggers}
            grants={grants}
          dependencies={dependencies}
          onRecompile={(object) => void recompileObject(object)}
          onToggleTrigger={(trigger) => void toggleTrigger(trigger)}
          onToggleConstraint={(constraint) => void toggleConstraint(constraint)}
          editable={editTable !== null}
          pendingEdits={pendingEdits.length}
          onSaveEdits={() => void saveEdits()}
          onCancelEdits={discardEdits}
          qbe={qbe}
          onQbe={setQbe}
          onQbeApply={applyQbe}
          />

        </main>
      </div>

      {showStatus && (
        <StatusBar
          dot={isConnected ? "ok" : ""}
          connLabel={
            isConnected
              ? `${draft.user}${draft.alias ? `@${draft.alias}` : ""}`
              : ""
          }
          environment={isConnected ? environment : null}
          transactionPending={transactionPending}
          message={status}
          eol={fileEol}
          cursor={cursor}
          server={isConnected ? (connected[selected] ?? "") : ""}
        />
      )}

      {cellView && (
        <div className="palette-backdrop" onClick={() => setCellView(null)}>
          <div className="palette" onClick={(event) => event.stopPropagation()}>
            <div className="panel-title">
              {cellView.column || t("cellViewer")}
              {editTable ? ` · ${t("editCell")}` : ""}
            </div>
            {editTable ? (
              <input
                autoFocus
                className="edit-input"
                value={editValue}
                aria-label={t("editCell")}
                onChange={(event) => setEditValue(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") stageEdit(cellView, editValue);
                }}
              />
            ) : (
              <pre className="cell-value">{cellView.value}</pre>
            )}
            <div className="row">
              {editTable && (
                <>
                  <button onClick={() => stageEdit(cellView, editValue)}>{t("apply")}</button>
                  <button onClick={() => stageEdit(cellView, "")}>{t("setNull")}</button>
                </>
              )}
              <button onClick={() => void navigator.clipboard.writeText(cellView.value)}>
                {t("copyValue")}
              </button>
              <button onClick={() => setCellView(null)}>{t("close")}</button>
            </div>
          </div>
        </div>
      )}
      {helpOpen && (
        <div className="palette-backdrop" onClick={() => setHelpOpen(false)}>
          <div className="palette" onClick={(event) => event.stopPropagation()}>
            <div className="panel-title">{t("shortcuts")}</div>
            <div className="palette-list">
              {commands
                .filter((command) => command.name && command.keys.length > 0)
                .sort((a, b) => a.name.localeCompare(b.name))
                .map((command) => (
                  <button
                    key={command.id}
                    onClick={() => {
                      setHelpOpen(false);
                      executeCommand(command.name);
                    }}
                  >
                    <span>{command.name}</span>
                    <span className="keys">{formatKeys(command.keys)}</span>
                  </button>
                ))}
            </div>
            <div className="row">
              <button onClick={() => setHelpOpen(false)}>{t("close")}</button>
            </div>
          </div>
        </div>
      )}
      {settingsOpen && (
        <div
          className="palette-backdrop"
          onClick={() => setSettingsOpen(false)}
        >
          <div
            className="settings-modal"
            onClick={(event) => event.stopPropagation()}
          >
            <h2>
              {settingsSection === "file"
                ? t("fileSettings")
                : t("permanentSettings")}
            </h2>
            {settingsSection === "app" && (
              <label className="field-row">
                <span>{t("app.language")}</span>
                <select
                  value={language}
                  onChange={(event) =>
                    setLanguage(event.target.value as "es" | "en")
                  }
                >
                  <option value="es">Español</option>
                  <option value="en">English</option>
                </select>
              </label>
            )}
            {settingsSection === "app" && (
              <>
                <label className="check-inline">
                  <input
                    type="checkbox"
                    checked={haltOnError}
                    onChange={(event) => setHaltOnError(event.target.checked)}
                  />
                  {t("haltOnError")}
                </label>
                <label className="check-inline">
                  <input
                    type="checkbox"
                    checked={substitution}
                    onChange={(event) => setSubstitution(event.target.checked)}
                  />
                  {t("substitution")}
                </label>
              </>
            )}
            <fieldset className="settings-eol">
              <legend>{t("lineEnding")}</legend>
              {(
                [
                  ["\n", "LF"],
                  ["\r\n", "CRLF"],
                  ["\r", "CR"],
                ] as const
              ).map(([value, label]) => (
                <label key={label}>
                  <input
                    type="radio"
                    name="eol"
                    checked={fileEol === value}
                    onChange={() => setFileEol(value)}
                  />
                  {label}
                </label>
              ))}
            </fieldset>
            <div className="row">
              <button className="plain" onClick={() => setSettingsOpen(false)}>
                {t("close")}
              </button>
            </div>
          </div>
        </div>
      )}
      {outlineOpen && (
        <div className="palette-backdrop" onClick={() => setOutlineOpen(false)}>
          <div
            className="settings-modal"
            onClick={(event) => event.stopPropagation()}
          >
            <h2>{t("outline")}</h2>
            {outline.length === 0 ? (
              <p className="no-results">{t("noOutline")}</p>
            ) : (
              <div className="palette-list">
                {outline.map((entry) => (
                  <button
                    key={`${entry.line}-${entry.label}`}
                    onClick={() => {
                      editorRef.current?.goToLineNumber(entry.line);
                      setOutlineOpen(false);
                    }}
                  >
                    <span>{entry.label}</span>
                    <span className="keys">{entry.line}</span>
                  </button>
                ))}
              </div>
            )}
            <div className="row">
              <button className="plain" onClick={() => setOutlineOpen(false)}>
                {t("close")}
              </button>
            </div>
          </div>
        </div>
      )}
      {filePanelOpen && (
        <div
          className="palette-backdrop"
          onClick={() => setFilePanelOpen(false)}
        >
          <div
            className="settings-modal"
            onClick={(event) => event.stopPropagation()}
          >
            <h2>{t("filePanel")}</h2>
            <table className="grid">
              <tbody>
                <tr>
                  <td>{t("name")}</td>
                  <td>{filePath || t("untitled")}</td>
                </tr>
                <tr>
                  <td>{t("lineEnding")}</td>
                  <td>
                    {fileEol === "\r\n"
                      ? "CRLF"
                      : fileEol === "\r"
                        ? "CR"
                        : "LF"}
                  </td>
                </tr>
                <tr>
                  <td>{t("lines")}</td>
                  <td>{sql.split("\n").length}</td>
                </tr>
                <tr>
                  <td>{t("characters")}</td>
                  <td>{sql.length}</td>
                </tr>
                <tr>
                  <td>{t("save")}</td>
                  <td>{dirty ? t("on") : t("off")}</td>
                </tr>
              </tbody>
            </table>
            <div className="row">
              <button className="plain" onClick={() => setFilePanelOpen(false)}>
                {t("close")}
              </button>
            </div>
          </div>
        </div>
      )}
      {findInFilesOpen && (
        <div
          className="palette-backdrop"
          onClick={() => setFindInFilesOpen(false)}
        >
          <div
            className="settings-modal"
            onClick={(event) => event.stopPropagation()}
          >
            <h2>{t("findInFiles")}</h2>
            <input
              autoFocus
              value={findQuery}
              placeholder={t("search")}
              onChange={(event) => setFindQuery(event.target.value)}
            />
            {findQuery && findMatches.length === 0 ? (
              <p className="no-results">{t("noResultsSearch")}</p>
            ) : (
              <div className="palette-list">
                {findMatches.map((match) => (
                  <button
                    key={match.line}
                    onClick={() => {
                      editorRef.current?.goToLineNumber(match.line);
                      setFindInFilesOpen(false);
                    }}
                  >
                    <span>{match.text || "(blank)"}</span>
                    <span className="keys">{match.line}</span>
                  </button>
                ))}
              </div>
            )}
            <div className="row">
              <button
                className="plain"
                onClick={() => setFindInFilesOpen(false)}
              >
                {t("close")}
              </button>
            </div>
          </div>
        </div>
      )}
      {newExtendedOpen && (
        <div
          className="palette-backdrop"
          onClick={() => setNewExtendedOpen(false)}
        >
          <div
            className="settings-modal"
            onClick={(event) => event.stopPropagation()}
          >
            <h2>{t("newFromTemplate")}</h2>
            <div className="palette-list">
              <button onClick={() => void createFromTemplate(null)}>
                <span>{t("blank")}</span>
                <span className="keys">{t("newFile")}</span>
              </button>
              {snippetNames.map((name) => (
                <button key={name} onClick={() => void createFromTemplate(name)}>
                  <span>{name}</span>
                  <span className="keys">{t("insert")}</span>
                </button>
              ))}
            </div>
            <div className="row">
              <button
                className="plain"
                onClick={() => setNewExtendedOpen(false)}
              >
                {t("close")}
              </button>
            </div>
          </div>
        </div>
      )}
      {openFilesOpen && (
        <div
          className="palette-backdrop"
          onClick={() => setOpenFilesOpen(false)}
        >
          <div
            className="settings-modal"
            onClick={(event) => event.stopPropagation()}
          >
            <h2>{t("openFiles")}</h2>
            <div className="palette-list">
              <button
                onClick={() => {
                  setOpenFilesOpen(false);
                  if (confirmDiscard()) newFile();
                }}
              >
                <span>
                  {filePath
                    ? filePath.split(/[\\/]/).pop()
                    : t("untitled")}
                </span>
                <span className="keys">{t("close")}</span>
              </button>
            </div>
            <div className="row">
              <button className="plain" onClick={() => setOpenFilesOpen(false)}>
                {t("close")}
              </button>
            </div>
          </div>
        </div>
      )}
      {snippetsOpen && (
        <div
          className="palette-backdrop"
          onClick={() => setSnippetsOpen(false)}
        >
          <div
            className="settings-modal"
            onClick={(event) => event.stopPropagation()}
          >
            <h2>{t("snippets")}</h2>
            {snippetNames.length === 0 ? (
              <p className="no-results">{t("noTemplates")}</p>
            ) : (
              <div className="palette-list">
                {snippetNames.map((name) => (
                  <button key={name} onClick={() => void insertSnippet(name)}>
                    <span>{name}</span>
                    <span className="keys">{t("insert")}</span>
                  </button>
                ))}
              </div>
            )}
            <div className="row">
              <button className="plain" onClick={() => setSnippetsOpen(false)}>
                {t("close")}
              </button>
            </div>
          </div>
        </div>
      )}
      {menu && (
        <>
          <div
            className="menu-backdrop"
            onClick={() => setMenu(null)}
            onContextMenu={(event) => {
              event.preventDefault();
              setMenu(null);
            }}
          />
          <div
            className="context-menu"
            style={{ left: menu.x, top: menu.y }}
          >
            <button
              onClick={() => {
                setMenu(null);
                void openObject(menu.object);
              }}
            >
              {t("object")}
            </button>
            <button
              onClick={() => {
                setMenu(null);
                queryObject(menu.object);
              }}
            >
              {t("query")}
            </button>
            <button
              onClick={() => {
                setMenu(null);
                countObject(menu.object);
              }}
            >
              {t("count")}
            </button>
            <button
              onClick={() => {
                setMenu(null);
                void loadDdlFor(menu.object);
              }}
            >
              {t("loadDdl")}
            </button>
            <button
              onClick={() => {
                setMenu(null);
                void showDependencies(menu.object);
              }}
            >
              {t("dependencies")}
            </button>
            <button
              onClick={() => {
                setMenu(null);
                void recompileObject(menu.object);
              }}
            >
              {t("recompile")}
            </button>
            {menu.object.objectType === "TRIGGER" && (
              <button
                onClick={() => {
                  setMenu(null);
                  void toggleTrigger(menu.object);
                }}
              >
                {/^ENABLED$/.test(menu.object.status) ? t("disable") : t("enable")}
              </button>
            )}
            <button
              onClick={() => {
                setMenu(null);
                void copyName(menu.object);
              }}
            >
              {t("copy")}
            </button>
          </div>
        </>
      )}
    </div>
  );
}
