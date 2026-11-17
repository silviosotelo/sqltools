import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";

import { Editor, type EditorHandle } from "./Editor";
import { api } from "./api";
import { Icon } from "./icons";
import { useTranslation } from "./i18n";
import type {
  ColumnInfo,
  ColumnMeta,
  CommandSpec,
  ConnectionConfig,
  ConstraintInfo,
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

type ObjectTab = "columns" | "constraints" | "indexes" | "triggers" | "grants";

type Draft = {
  id: string;
  label: string;
  user: string;
  alias: string;
  connect: string;
  tnsAdmin: string;
  ociDir: string;
  mode: "NORMAL" | "SYSDBA" | "SYSOPER";
  readOnly: boolean;
  prefetch: number;
  savePassword: boolean;
};

type Panel = "results" | "history" | "plan" | "object" | "stats" | "output";

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

const ROW_HEIGHT = 24;
const OVERSCAN = 8;
const VISIBLE_ROWS = 60;

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
  };
}

function cellText(value: Value): string {
  if (value.kind === "null") return "";
  if (value.kind === "binary") {
    return value.value.map((byte) => byte.toString(16).padStart(2, "0")).join("");
  }
  return value.value;
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
  const [connections, setConnections] = useState<ConnectionConfig[]>([]);
  const [selected, setSelected] = useState("");
  const [connected, setConnected] = useState<Record<string, string>>({});
  const [draft, setDraft] = useState<Draft>(emptyDraft);
  const [password, setPassword] = useState("");
  const [sql, setSql] = useState("select * from dual");
  const [cleanSql, setCleanSql] = useState("select * from dual");
  const [filePath, setFilePath] = useState("");
  const [fileEol, setFileEol] = useState("\n");
  const [cleanEol, setCleanEol] = useState("\n");
  const [columns, setColumns] = useState<ColumnMeta[]>([]);
  const [rows, setRows] = useState<Value[][]>([]);
  const [scrollTop, setScrollTop] = useState(0);
  const [running, setRunning] = useState(false);
  const [status, setStatus] = useState("");
  const [tables, setTables] = useState<string[]>([]);
  const [historyEntries, setHistoryEntries] = useState<HistoryEntry[]>([]);
  const [plan, setPlan] = useState("");
  const [panel, setPanel] = useState<Panel>("results");
  const [schema, setSchema] = useState("");
  const [objectType, setObjectType] = useState("");
  const [objectFilter, setObjectFilter] = useState("");
  const [objects, setObjects] = useState<ObjectInfo[]>([]);
  const [selectedObject, setSelectedObject] = useState<ObjectInfo | null>(null);
  const [objectColumns, setObjectColumns] = useState<ColumnInfo[]>([]);
  const [objectTab, setObjectTab] = useState<ObjectTab>("columns");
  const [constraints, setConstraints] = useState<ConstraintInfo[]>([]);
  const [indexes, setIndexes] = useState<IndexInfo[]>([]);
  const [triggers, setTriggers] = useState<TriggerInfo[]>([]);
  const [grants, setGrants] = useState<GrantInfo[]>([]);
  const [menu, setMenu] = useState<{
    object: ObjectInfo;
    x: number;
    y: number;
  } | null>(null);
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
  const [profiles, setProfiles] = useState<string[]>([]);
  const [profile, setProfile] = useState("");
  const [profileOptions, setProfileOptions] = useState<FormatOptions | null>(
    null,
  );
  const [showStatus, setShowStatus] = useState(true);
  const [showConnections, setShowConnections] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [installing, setInstalling] = useState(false);
  const [haltOnError, setHaltOnErrorState] = useState(() => {
    try {
      return localStorage.getItem("sqlt.haltOnError") !== "false";
    } catch {
      return true;
    }
  });
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

  const isConnected = Boolean(selected && connected[selected]);

  const refresh = async () => {
    setConnections(await api.listConnections());
  };

  useEffect(() => {
    void refresh();
  }, []);

  useEffect(() => {
    if (connections.length === 0) setShowConnections(true);
  }, [connections.length]);

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

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (
        (event.ctrlKey || event.metaKey) &&
        event.shiftKey &&
        event.key.toLowerCase() === "p"
      ) {
        event.preventDefault();
        setPaletteOpen((open) => !open);
      }
      if (event.key === "Escape") setPaletteOpen(false);
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
    setObjects([]);
    setSelectedObject(null);
    setObjectColumns([]);
    setConstraints([]);
    setIndexes([]);
    setTriggers([]);
    setGrants([]);
    setObjectTab("columns");
    setMenu(null);
    setStats([]);
  };

  const columnCache = useRef(new Map<string, string[]>());
  const editorRef = useRef<EditorHandle | null>(null);

  const loadColumnsFor = async (table: string) => {
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
    try {
      await api.saveConnection(toConfig(draft), password);
      setPassword("");
      await refresh();
      setSelected(draft.id);
      setShowConnections(false);
      setStatus(t("save"));
    } catch (error) {
      setStatus(String(error));
    }
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
    try {
      const version = await api.connect(selected);
      setConnected((current) => ({ ...current, [selected]: version }));
      setStatus(`${t("connected")}: ${version}`);
      setTables(await api.listTableNames(selected));
      setShowConnections(false);
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
      setObjects([]);
      setSelectedObject(null);
      setObjectColumns([]);
      setStats([]);
      setStatus(t("disconnect"));
    } catch (error) {
      setStatus(String(error));
    }
  };

  const run = async (text = sql, offset: number | null = null) => {
    if (!isConnected) {
      setStatus(t("noConnection"));
      return;
    }
    // The session runs one script at a time; ignore repeats while busy.
    if (running) {
      setStatus(t("alreadyRunning"));
      return;
    }
    setRows([]);
    setColumns([]);
    setOutput([]);
    setScriptErrors([]);
    setErrorIndex(-1);
    setScrollTop(0);
    // Errors from this run are only valid while the text it ran on is intact.
    const runVersion = bufferVersion.current;
    setRunning(true);
    setStatus("");
    setPanel("results");
    try {
      await api.execute(
        selected,
        text,
        5000,
        offset,
        haltOnError,
        (event: QueryEvent) => {
          if (event.type === "columns") {
            // Each statement of a script replaces the grid with its own result.
            setColumns(event.data);
            setRows([]);
          } else if (event.type === "batch") {
            setRows((current) => [...current, ...event.data.rows]);
          } else if (event.type === "output") {
            setOutput((current) => [...current, ...event.data]);
            setPanel("output");
          } else if (event.type === "statementerror") {
            if (bufferVersion.current === runVersion) {
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
            setRunning(false);
            setStatus(event.data);
          }
        },
      );
    } catch (error) {
      setRunning(false);
      setStatus(String(error));
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
      setStatus(t("commit"));
    } catch (error) {
      setStatus(String(error));
    }
  };

  const rollback = async () => {
    if (!selected) return;
    try {
      await api.rollback(selected);
      setStatus(t("rollback"));
    } catch (error) {
      setStatus(String(error));
    }
  };

  const showHistory = async () => {
    try {
      setHistoryEntries(await api.history());
      setPanel("history");
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
    try {
      setStats(await api.sessionStats(selected));
      setPanel("stats");
    } catch (error) {
      setStatus(String(error));
    }
  };

  const loadObjects = async (type: string = objectType) => {
    if (!isConnected) {
      setStatus(t("noConnection"));
      return;
    }
    try {
      setObjects(
        await api.listObjects(
          selected,
          schema.trim() || null,
          type.trim() || null,
        ),
      );
    } catch (error) {
      setStatus(String(error));
    }
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

  const chooseType = (type: string) => {
    setObjectType(type);
    if (isConnected) void loadObjects(type);
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

  const loadDdlFor = async (object: ObjectInfo) => {
    try {
      setSelectedObject(object);
      const owner = schema.trim() || null;
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
      setSql(ddl);
      setPanel("results");
      setStatus(t("loadDdl"));
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

  const confirmDiscard = () => !dirty || window.confirm(t("discardChanges"));

  const newFile = () => {
    if (!confirmDiscard()) return;
    setSql("");
    setCleanSql("");
    setFilePath("");
    setFileEol("\n");
    setCleanEol("\n");
    setColumns([]);
    setRows([]);
    setPanel("results");
    setStatus("");
  };

  const openFile = async () => {
    if (!confirmDiscard()) return;
    try {
      const path = await api.pickSqlFile();
      if (!path) return;
      const file = await api.readSqlFile(path);
      setSql(file.text);
      setCleanSql(file.text);
      setFilePath(path);
      setFileEol(file.eol);
      setCleanEol(file.eol);
      setStatus(path);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const saveFile = async (saveAs = false) => {
    try {
      let path = filePath;
      if (saveAs || !path) {
        const picked = await api.pickSqlSavePath(filePath || "query.sql");
        if (!picked) return;
        path = picked;
      }
      await api.writeSqlFile(path, sql, fileEol);
      setFilePath(path);
      setCleanSql(sql);
      setCleanEol(fileEol);
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
    if (!confirmDiscard()) return;
    try {
      const file = await api.readSqlFile(path);
      setSql(file.text);
      setCleanSql(file.text);
      setFilePath(path);
      setFileEol(file.eol);
      setCleanEol(file.eol);
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
    try {
      const file = await api.readSqlFile(filePath);
      setSql(file.text);
      setCleanSql(file.text);
      setFileEol(file.eol);
      setCleanEol(file.eol);
      setStatus(filePath);
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
      setTables(await api.listTableNames(selected));
      setStatus(`${t("connected")}: ${version}`);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const copyWorkspace = async () => {
    try {
      const workspace: Workspace = {
        version: 1,
        files: [{ name: "query.sql", sql }],
        active: 0,
      };
      await navigator.clipboard.writeText(JSON.stringify(workspace));
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
      const file = workspace.files?.[workspace.active] ?? workspace.files?.[0];
      if (!file || typeof file.sql !== "string") {
        setStatus(t("workspaceLoad"));
        return;
      }
      setSql(file.sql);
      setCleanSql(file.sql);
      setFilePath("");
      setFileEol("\n");
      setCleanEol("\n");
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
    try {
      setSql(
        await api.loadDdl(
          selected,
          schema.trim() || null,
          selectedObject.name,
          selectedObject.objectType,
        ),
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
      await api.saveWorkspace({
        version: 1,
        files: [{ name: "query.sql", sql }],
        active: 0,
      });
      setStatus(t("workspaceSave"));
    } catch (error) {
      setStatus(String(error));
    }
  };

  const saveWorkspaceAs = async () => {
    try {
      const path = await api.pickWorkspaceSavePath("workspace.json");
      if (!path) return;
      await api.saveWorkspaceTo(path, {
        version: 1,
        files: [{ name: "query.sql", sql }],
        active: 0,
      });
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

  const openNewExtended = async () => {
    try {
      setSnippetNames(await api.listTemplates());
      setNewExtendedOpen(true);
    } catch (error) {
      setStatus(String(error));
    }
  };

  const createFromTemplate = async (name: string | null) => {
    if (!confirmDiscard()) return;
    setNewExtendedOpen(false);
    setSql("");
    setCleanSql("");
    setFilePath("");
    setFileEol("\n");
    setCleanEol("\n");
    setColumns([]);
    setRows([]);
    setOutput([]);
    setScriptErrors([]);
    setErrorIndex(-1);
    setPanel("results");
    setStatus("");
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
      void run(sql, current.start);
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
      const file = workspace.files[workspace.active] ?? workspace.files[0];
      if (file) {
        setSql(file.sql);
        setCleanSql(file.sql);
        setFilePath("");
        setFileEol("\n");
        setCleanEol("\n");
      }
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
      case "File.CloseAll":
        newFile();
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
      case "Script.LoadDDL":
        if (selectedObject) void loadDdlFor(selectedObject);
        else setStatus(t("selectObject"));
        break;
      case "View.ObjectList":
      case "View.ObjectTree":
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
  };

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
      const command = filteredCommands[paletteIndex];
      if (command) executeCommand(command.name);
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

  const start = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
  const end = Math.min(rows.length, start + VISIBLE_ROWS + OVERSCAN * 2);

  const shownObjects = objects.filter(
    (object) =>
      !objectFilter ||
      object.name.toLowerCase().includes(objectFilter.toLowerCase()),
  );

  return (
    <div className="app">
      <header className="topbar">
        <div className="toolbar">
          <div className="brand-inline">
            <Icon name="database" size={18} />
            <strong>{t("app.title")}</strong>
            <span className="brand-sub">{t("app.subtitle")}</span>
          </div>
          <span className="tb-sep" />
          <button
            className="tb"
            title={t("execute")}
            aria-label={t("execute")}
            onClick={() => void run()}
            disabled={!isConnected || running}
          >
            <Icon name="run" />
          </button>
          <button
            className="tb"
            title={t("cancel")}
            aria-label={t("cancel")}
            onClick={() => void cancel()}
            disabled={!running}
          >
            <Icon name="stop" />
          </button>
          <button
            className="tb"
            title={t("commit")}
            aria-label={t("commit")}
            onClick={() => void commit()}
            disabled={!isConnected || running}
          >
            <Icon name="commit" />
          </button>
          <button
            className="tb"
            title={t("rollback")}
            aria-label={t("rollback")}
            onClick={() => void rollback()}
            disabled={!isConnected || running}
          >
            <Icon name="rollback" />
          </button>
          <span className="tb-sep" />
          <button
            className="tb"
            title={t("format")}
            aria-label={t("format")}
            onClick={() => void formatSql()}
            disabled={!sql.trim()}
          >
            <Icon name="format" />
          </button>
          <button
            className="tb"
            title={t("history")}
            aria-label={t("history")}
            onClick={() => void showHistory()}
          >
            <Icon name="history" />
          </button>
          <button
            className="tb"
            title={t("explain")}
            aria-label={t("explain")}
            onClick={() => void explain()}
            disabled={!isConnected}
          >
            <Icon name="plan" />
          </button>
          <button
            className="tb"
            title={t("stats")}
            aria-label={t("stats")}
            onClick={() => void showStats()}
            disabled={!isConnected}
          >
            <Icon name="stats" />
          </button>
          <button
            className="tb"
            title={t("exportCsv")}
            aria-label={t("exportCsv")}
            onClick={() => void exportCsv()}
            disabled={!columns.length && !isConnected}
          >
            <Icon name="export" />
          </button>
          <span className="tb-sep" />
          <button
            className="tb"
            title={t("workspaceSave")}
            aria-label={t("workspaceSave")}
            onClick={() => void saveWorkspace()}
          >
            <Icon name="save" />
          </button>
          <button
            className="tb"
            title={t("workspaceLoad")}
            aria-label={t("workspaceLoad")}
            onClick={() => void loadWorkspace()}
          >
            <Icon name="open" />
          </button>
          <button
            className="tb"
            title={t("commandPalette")}
            aria-label={t("commandPalette")}
            onClick={() => setPaletteOpen(true)}
          >
            <Icon name="commands" />
          </button>
          <span className="toolbar-spacer" />
          {updateInfo && (
            <button
              className="tb"
              title={`${t("installUpdate")}: ${updateInfo.version}`}
              aria-label={t("installUpdate")}
              onClick={() => void installUpdate()}
              disabled={installing}
            >
              <Icon name="update" />
            </button>
          )}
          <button
            className="tb"
            title={t("checkUpdates")}
            aria-label={t("checkUpdates")}
            onClick={() => void checkUpdates()}
          >
            <Icon name="update" />
          </button>
        </div>

        <div className="toolbar">
          <select
            className="connection-select"
            value={selected}
            aria-label={t("connections")}
            onChange={(event) => selectConnection(event.target.value)}
          >
            <option value="">—</option>
            {connections.map((connection) => (
              <option key={connection.id} value={connection.id}>
                {connection.label} ({connection.user})
              </option>
            ))}
          </select>
          <button
            className="tb"
            title={t("connect")}
            aria-label={t("connect")}
            onClick={() => void connect()}
            disabled={!selected || isConnected}
          >
            <Icon name="connect" />
          </button>
          <button
            className="tb"
            title={t("disconnect")}
            aria-label={t("disconnect")}
            onClick={() => void disconnect()}
            disabled={!isConnected}
          >
            <Icon name="disconnect" />
          </button>
          <label className="check-inline">
            <input
              type="checkbox"
              checked={draft.readOnly}
              onChange={(event) => update("readOnly", event.target.checked)}
            />
            {t("readOnly")}
          </label>
          <button
            className="tb"
            title={t("refresh")}
            aria-label={t("refresh")}
            onClick={() => void refreshObjectCache()}
            disabled={!isConnected}
          >
            <Icon name="refresh" />
          </button>
          <button
            className={showConnections ? "tb active" : "tb"}
            title={t("connections")}
            aria-label={t("connections")}
            onClick={() => setShowConnections((visible) => !visible)}
          >
            <Icon name="settings" />
          </button>
          <span className="toolbar-spacer" />
          <select
            className="language-select"
            value={language}
            aria-label={t("app.language")}
            onChange={(event) =>
              setLanguage(event.target.value as "es" | "en")
            }
          >
            <option value="es">Español</option>
            <option value="en">English</option>
          </select>
          <span
            className={isConnected ? "connection-chip on" : "connection-chip"}
            title={isConnected ? connected[selected] : t("noConnection")}
          >
            {isConnected
              ? `${t("connected")}: ${draft.user}${
                  draft.alias ? `@${draft.alias}` : ""
                }`
              : t("noConnection")}
          </span>
        </div>
      </header>

      <div className="body">
        <aside className="objects-panel">
          <div className="panel-title">{t("objects")}</div>
          <div className="object-filter">
            <input
              value={schema}
              placeholder={t("schema")}
              title={t("schema")}
              onChange={(event) => setSchema(event.target.value)}
            />
            <input
              value={objectFilter}
              placeholder={t("filter")}
              title={t("filter")}
              onChange={(event) => setObjectFilter(event.target.value)}
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
          </div>
          <div className="type-grid">
            {objectTypes.map((type) => (
              <button
                key={type.value || "all"}
                className={
                  objectType === type.value ? "type-button active" : "type-button"
                }
                onClick={() => chooseType(type.value)}
                disabled={!isConnected}
              >
                {t(type.key)}
              </button>
            ))}
          </div>
          <div className="object-scroll">
            <table className="object-table">
              <thead>
                <tr>
                  <th>{t("name")}</th>
                  <th>{t("objectType")}</th>
                </tr>
              </thead>
              <tbody>
                {shownObjects.map((object) => (
                  <tr
                    key={`${object.objectType}-${object.name}`}
                    className={
                      selectedObject?.name === object.name &&
                      selectedObject?.objectType === object.objectType
                        ? "active"
                        : ""
                    }
                    title={object.status}
                    onContextMenu={(event) => {
                      event.preventDefault();
                      setMenu({ object, x: event.clientX, y: event.clientY });
                    }}
                    onClick={() => void openObject(object)}
                  >
                    <td>{object.name}</td>
                    <td>{object.objectType}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <details
            className="side-section"
            open={showConnections}
            onToggle={(event) => setShowConnections(event.currentTarget.open)}
          >
            <summary>{t("connections")}</summary>
            <div className="section-body">
              <label className="field-row">
                <span>{t("label")}</span>
                <input
                  value={draft.label}
                  onChange={(event) => update("label", event.target.value)}
                />
              </label>
              <label className="field-row">
                <span>{t("user")}</span>
                <input
                  value={draft.user}
                  onChange={(event) => update("user", event.target.value)}
                />
              </label>
              <label className="field-row">
                <span>{t("password")}</span>
                <input
                  type="password"
                  value={password}
                  onChange={(event) => setPassword(event.target.value)}
                />
              </label>
              <label className="field-row">
                <span>{t("alias")}</span>
                <input
                  value={draft.alias}
                  onChange={(event) => update("alias", event.target.value)}
                />
              </label>
              <label className="field-row">
                <span>{t("connectString")}</span>
                <input
                  value={draft.connect}
                  placeholder="//host:1521/service"
                  onChange={(event) => update("connect", event.target.value)}
                />
              </label>
              <label className="field-row">
                <span>{t("tnsAdmin")}</span>
                <input
                  value={draft.tnsAdmin}
                  onChange={(event) => update("tnsAdmin", event.target.value)}
                />
              </label>
              <label className="field-row">
                <span>{t("ociDir")}</span>
                <input
                  value={draft.ociDir}
                  onChange={(event) => update("ociDir", event.target.value)}
                />
              </label>
              <label className="field-row">
                <span>{t("mode")}</span>
                <select
                  value={draft.mode}
                  onChange={(event) =>
                    update("mode", event.target.value as Draft["mode"])
                  }
                >
                  <option value="NORMAL">NORMAL</option>
                  <option value="SYSDBA">SYSDBA</option>
                  <option value="SYSOPER">SYSOPER</option>
                </select>
              </label>
              <label className="field-row">
                <span>{t("prefetch")}</span>
                <input
                  type="number"
                  min={1}
                  value={draft.prefetch}
                  onChange={(event) =>
                    update("prefetch", Number(event.target.value) || 1000)
                  }
                />
              </label>
              <label className="check-inline">
                <input
                  type="checkbox"
                  checked={draft.savePassword}
                  onChange={(event) =>
                    update("savePassword", event.target.checked)
                  }
                />
                {t("savePassword")}
              </label>
              <div className="row">
                <button className="plain" onClick={() => void save()}>
                  {t("save")}
                </button>
                <button
                  className="plain"
                  onClick={() => void remove()}
                  disabled={!selected}
                >
                  {t("delete")}
                </button>
                <button className="plain" onClick={() => void importLegacy()}>
                  {t("importLegacy")}
                </button>
              </div>
            </div>
          </details>

          <details className="side-section">
            <summary>{t("formatterProfiles")}</summary>
            <div className="section-body">
              <label className="field-row">
                <span>{t("profile")}</span>
                <select
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

        <main className="workspace">
          <div className="editor-shell">
            <div className="editor-tabbar">
              <div className="editor-tab">
                <span>
                  {dirty ? "*" : ""}
                  {filePath ? filePath.split(/[\\/]/).pop() : "SQL"}
                </span>
                <span className="tab-conn">
                  {isConnected
                    ? `${draft.user}${draft.alias ? `@${draft.alias}` : ""}`
                    : t("noConnection")}
                </span>
              </div>
            </div>
            <Editor
              ref={editorRef}
              initialValue={sql}
              tables={tables}
              loadColumns={loadColumnsFor}
              onChange={setSql}
              onExecute={(text, offset) => void run(text, offset)}
            />
          </div>

          <div className="panel-bar">
            <nav className="tabs">
              <button
                className={panel === "results" ? "active" : ""}
                onClick={() => setPanel("results")}
              >
                {t("results")}
              </button>
              <button
                className={panel === "history" ? "active" : ""}
                onClick={() => void showHistory()}
              >
                {t("history")}
              </button>
              <button
                className={panel === "plan" ? "active" : ""}
                onClick={() => setPanel("plan")}
              >
                {t("plan")}
              </button>
              <button
                className={panel === "object" ? "active" : ""}
                onClick={() => setPanel("object")}
              >
                {t("object")}
              </button>
              <button
                className={panel === "stats" ? "active" : ""}
                onClick={() => void showStats()}
              >
                {t("stats")}
              </button>
              <button
                className={panel === "output" ? "active" : ""}
                onClick={() => setPanel("output")}
              >
                {t("output")}
              </button>
            </nav>
            <button
              className="tb"
              title={t("exportCsv")}
              aria-label={t("exportCsv")}
              onClick={() => void exportCsv()}
              disabled={!columns.length && !isConnected}
            >
              <Icon name="export" />
            </button>
            <button
              className="tb"
              title={t("clear")}
              aria-label={t("clear")}
              onClick={() => {
                setRows([]);
                setColumns([]);
                setStatus("");
              }}
            >
              <Icon name="clear" />
            </button>
            <span className="hint">{t("bookmarkHint")}</span>
          </div>

          <section className="content">
            {panel === "results" && (
              <div
                className="results"
                onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
              >
                {columns.length === 0 ? (
                  <p className="no-results">{t("noResults")}</p>
                ) : (
                <table className="grid">
                  <thead>
                    <tr>
                      <th className="row-number">#</th>
                      {columns.map((column) => (
                        <th key={column.name} title={column.typeName}>
                          {column.name}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {start > 0 && (
                      <tr
                        className="spacer"
                        style={{ height: start * ROW_HEIGHT }}
                      >
                        <td colSpan={Math.max(columns.length + 1, 1)} />
                      </tr>
                    )}
                    {rows.slice(start, end).map((row, index) => (
                      <tr key={start + index} style={{ height: ROW_HEIGHT }}>
                        <td className="row-number">{start + index + 1}</td>
                        {row.map((value, columnIndex) => (
                          <td
                            key={columnIndex}
                            className={value.kind === "null" ? "null" : ""}
                          >
                            {cellText(value)}
                          </td>
                        ))}
                      </tr>
                    ))}
                    {end < rows.length && (
                      <tr
                        className="spacer"
                        style={{ height: (rows.length - end) * ROW_HEIGHT }}
                      >
                        <td colSpan={Math.max(columns.length + 1, 1)} />
                      </tr>
                    )}
                  </tbody>
                </table>
                )}
              </div>
            )}

            {panel === "history" && (
              <div className="history">
                {historyEntries.map((entry, index) => (
                  <button
                    key={index}
                    className="history-row"
                    onClick={() => setSql(entry.sql)}
                  >
                    <span>
                      {new Date(entry.startedAt * 1000).toLocaleString()}
                    </span>
                    <code>{entry.sql}</code>
                    <span>
                      {entry.rows} {entry.rows === 1 ? t("row") : t("rows")} ·{" "}
                      {entry.elapsedMs} ms
                      {entry.error ? ` · ${entry.error}` : ""}
                    </span>
                  </button>
                ))}
              </div>
            )}

            {panel === "plan" && <pre className="plan">{plan}</pre>}

            {panel === "object" && (
              <div className="object-panel">
                {selectedObject ? (
                  <>
                    <header className="object-header">
                      <strong>{selectedObject.name}</strong>
                      <span>{selectedObject.objectType}</span>
                      <button
                        className="plain"
                        onClick={() => void loadDdlFor(selectedObject)}
                      >
                        {t("loadDdl")}
                      </button>
                    </header>
                    <nav className="object-tabs">
                      {(
                        [
                          "columns",
                          "constraints",
                          "indexes",
                          "triggers",
                          "grants",
                        ] as ObjectTab[]
                      ).map((tab) => (
                        <button
                          key={tab}
                          className={objectTab === tab ? "active" : ""}
                          onClick={() => void loadDetail(tab)}
                        >
                          {t(tab)}
                        </button>
                      ))}
                    </nav>
                    {objectTab === "columns" &&
                      (objectColumns.length > 0 ? (
                        <table className="grid">
                          <thead>
                            <tr>
                              <th>{t("columns")}</th>
                              <th>{t("dataType")}</th>
                              <th>{t("nullable")}</th>
                              <th>{t("defaultValue")}</th>
                            </tr>
                          </thead>
                          <tbody>
                            {objectColumns.map((column) => (
                              <tr key={column.name}>
                                <td>{column.name}</td>
                                <td>{column.dataType}</td>
                                <td>{column.nullable ? "Y" : "N"}</td>
                                <td>{column.dataDefault ?? ""}</td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      ) : (
                        <p className="hint">{selectedObject.status}</p>
                      ))}
                    {objectTab === "constraints" && (
                      <table className="grid">
                        <thead>
                          <tr>
                            <th>{t("constraints")}</th>
                            <th>{t("objectType")}</th>
                            <th>Status</th>
                            <th>{t("columns")}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {constraints.map((constraint) => (
                            <tr key={constraint.name}>
                              <td>{constraint.name}</td>
                              <td>{constraint.kind}</td>
                              <td>{constraint.status}</td>
                              <td>{constraint.columns.join(", ")}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    )}
                    {objectTab === "indexes" && (
                      <table className="grid">
                        <thead>
                          <tr>
                            <th>{t("indexes")}</th>
                            <th>{t("uniqueness")}</th>
                            <th>Status</th>
                            <th>{t("columns")}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {indexes.map((index) => (
                            <tr key={index.name}>
                              <td>{index.name}</td>
                              <td>{index.uniqueness}</td>
                              <td>{index.status}</td>
                              <td>{index.columns.join(", ")}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    )}
                    {objectTab === "triggers" && (
                      <table className="grid">
                        <thead>
                          <tr>
                            <th>{t("triggers")}</th>
                            <th>{t("objectType")}</th>
                            <th>{t("event")}</th>
                            <th>Status</th>
                          </tr>
                        </thead>
                        <tbody>
                          {triggers.map((trigger) => (
                            <tr key={trigger.name}>
                              <td>{trigger.name}</td>
                              <td>{trigger.triggerType}</td>
                              <td>{trigger.event}</td>
                              <td>{trigger.status}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    )}
                    {objectTab === "grants" && (
                      <table className="grid">
                        <thead>
                          <tr>
                            <th>{t("grantee")}</th>
                            <th>{t("privilege")}</th>
                            <th>{t("grantable")}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {grants.map((grant, index) => (
                            <tr key={`${grant.grantee}-${index}`}>
                              <td>{grant.grantee}</td>
                              <td>{grant.privilege}</td>
                              <td>{grant.grantable ? "Y" : "N"}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    )}
                  </>
                ) : (
                  <p className="hint">{t("selectObject")}</p>
                )}
              </div>
            )}

            {panel === "output" &&
              (output.length === 0 ? (
                <p className="no-results">{t("noOutput")}</p>
              ) : (
                <pre className="plan output">{output.join("\n")}</pre>
              ))}

            {panel === "stats" && (
              <table className="grid">
                <thead>
                  <tr>
                    <th>{t("stats")}</th>
                    <th>Value</th>
                  </tr>
                </thead>
                <tbody>
                  {stats.map(([name, value]) => (
                    <tr key={name}>
                      <td>{name}</td>
                      <td>{value}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </section>

          {showStatus && (
            <footer className="status-bar">
              <span>{status}</span>
              <span className="status-right">
                {isConnected ? t("connected") : t("noConnection")}
              </span>
            </footer>
          )}
        </main>
      </div>

      {paletteOpen && (
        <div
          className="palette-backdrop"
          onClick={() => setPaletteOpen(false)}
        >
          <div className="palette" onClick={(event) => event.stopPropagation()}>
            <input
              autoFocus
              value={paletteQuery}
              placeholder={t("searchCommands")}
              onChange={(event) => {
                setPaletteQuery(event.target.value);
                setPaletteIndex(0);
              }}
              onKeyDown={onPaletteKey}
            />
            <div className="palette-list">
              {filteredCommands.map((command, index) => (
                <button
                  key={command.id}
                  className={index === paletteIndex ? "active" : ""}
                  onClick={() => executeCommand(command.name)}
                >
                  <span>{command.name}</span>
                  <span className="keys">{command.keys[0] ?? ""}</span>
                  {!command.implemented && <em>{t("notImplemented")}</em>}
                </button>
              ))}
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
              <label className="check-inline">
                <input
                  type="checkbox"
                  checked={haltOnError}
                  onChange={(event) => setHaltOnError(event.target.checked)}
                />
                {t("haltOnError")}
              </label>
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
