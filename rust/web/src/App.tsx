import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { Editor, type EditorHandle } from "./Editor";
import { api } from "./api";
import { useTranslation } from "./i18n";
import type {
  ColumnInfo,
  ColumnMeta,
  CommandSpec,
  ConnectionConfig,
  ConstraintInfo,
  GrantInfo,
  HistoryEntry,
  IndexInfo,
  ObjectInfo,
  QueryEvent,
  StatEntry,
  TriggerInfo,
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

type Panel = "results" | "history" | "plan" | "object" | "stats";

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
    void api.listCommands().then(setCommands);
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

  const run = async (text = sql) => {
    if (!isConnected) {
      setStatus(t("noConnection"));
      return;
    }
    setRows([]);
    setColumns([]);
    setScrollTop(0);
    setRunning(true);
    setStatus("");
    setPanel("results");
    try {
      await api.execute(selected, text, 5000, (event: QueryEvent) => {
        if (event.type === "columns") {
          setColumns(event.data);
        } else if (event.type === "batch") {
          setRows((current) => [...current, ...event.data.rows]);
        } else if (event.type === "done") {
          setRunning(false);
          setStatus(`${event.data.rows} ${t("rows")} · ${event.data.elapsedMs} ms`);
        } else {
          setRunning(false);
          setStatus(event.data);
        }
      });
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

  const loadObjects = async () => {
    if (!isConnected) {
      setStatus(t("noConnection"));
      return;
    }
    try {
      setObjects(
        await api.listObjects(
          selected,
          schema.trim() || null,
          objectType.trim() || null,
        ),
      );
      setPanel("object");
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
      setSql(await api.formatSql(sql, true));
    } catch (error) {
      setStatus(String(error));
    }
  };
  const newFile = () => {
    setSql("");
    setColumns([]);
    setRows([]);
    setPanel("results");
    setStatus("");
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
      setStatus(await api.checkUpdates());
    } catch (error) {
      setStatus(String(error));
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

  const loadWorkspace = async () => {
    try {
      const workspace: Workspace = await api.loadWorkspace();
      const file = workspace.files[workspace.active] ?? workspace.files[0];
      if (file) setSql(file.sql);
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
    switch (name) {
      case "File.New":
        newFile();
        break;
      case "Session.Reconnect":
        void reconnect();
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
      case "Script.Execute":
      case "Script.ExecuteCurrent":
        void run();
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

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <strong>{t("app.title")}</strong>
          <span>{t("app.subtitle")}</span>
        </div>

        <label className="language">
          {t("app.language")}
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

        <h2>{t("connections")}</h2>
        <select
          className="connection-select"
          value={selected}
          onChange={(event) => selectConnection(event.target.value)}
        >
          <option value="">—</option>
          {connections.map((connection) => (
            <option key={connection.id} value={connection.id}>
              {connection.label} ({connection.user})
              {connected[connection.id] ? " ✓" : ""}
            </option>
          ))}
        </select>
        <div className="row">
          <button onClick={() => void connect()} disabled={!selected}>
            {t("connect")}
          </button>
          <button onClick={() => void disconnect()} disabled={!isConnected}>
            {t("disconnect")}
          </button>
        </div>

        <details open>
          <summary>{t("newConnection")}</summary>
          <label>
            {t("label")}
            <input
              value={draft.label}
              onChange={(event) => update("label", event.target.value)}
            />
          </label>
          <label>
            {t("user")}
            <input
              value={draft.user}
              onChange={(event) => update("user", event.target.value)}
            />
          </label>
          <label>
            {t("password")}
            <input
              type="password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
            />
          </label>
          <label>
            {t("alias")}
            <input
              value={draft.alias}
              onChange={(event) => update("alias", event.target.value)}
            />
          </label>
          <label>
            {t("connectString")}
            <input
              value={draft.connect}
              placeholder="//host:1521/service"
              onChange={(event) => update("connect", event.target.value)}
            />
          </label>
          <label>
            {t("tnsAdmin")}
            <input
              value={draft.tnsAdmin}
              onChange={(event) => update("tnsAdmin", event.target.value)}
            />
          </label>
          <label>
            {t("ociDir")}
            <input
              value={draft.ociDir}
              onChange={(event) => update("ociDir", event.target.value)}
            />
          </label>
          <label>
            {t("mode")}
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
          <div className="row">
            <label className="check">
              <input
                type="checkbox"
                checked={draft.readOnly}
                onChange={(event) => update("readOnly", event.target.checked)}
              />
              {t("readOnly")}
            </label>
            <label className="check">
              <input
                type="checkbox"
                checked={draft.savePassword}
                onChange={(event) =>
                  update("savePassword", event.target.checked)
                }
              />
              {t("savePassword")}
            </label>
          </div>
          <label>
            {t("prefetch")}
            <input
              type="number"
              min={1}
              value={draft.prefetch}
              onChange={(event) =>
                update("prefetch", Number(event.target.value) || 1000)
              }
            />
          </label>
          <div className="row">
            <button onClick={() => void save()}>{t("save")}</button>
            <button onClick={() => void remove()} disabled={!selected}>
              {t("delete")}
            </button>
            <button onClick={() => void importLegacy()}>
              {t("importLegacy")}
            </button>
          </div>
        </details>

        <details open>
          <summary>{t("objects")}</summary>
          <label>
            {t("schema")}
            <input
              value={schema}
              placeholder="(current)"
              onChange={(event) => setSchema(event.target.value)}
            />
          </label>
          <label>
            {t("objectType")}
            <select
              value={objectType}
              onChange={(event) => setObjectType(event.target.value)}
            >
              <option value="">{t("allTypes")}</option>
              <option value="TABLE">TABLE</option>
              <option value="VIEW">VIEW</option>
              <option value="PACKAGE">PACKAGE</option>
              <option value="PROCEDURE">PROCEDURE</option>
              <option value="FUNCTION">FUNCTION</option>
              <option value="TRIGGER">TRIGGER</option>
              <option value="SEQUENCE">SEQUENCE</option>
            </select>
          </label>
          <button onClick={() => void loadObjects()} disabled={!isConnected}>
            {t("loadObjects")}
          </button>
          <div className="object-list">
            {objects.map((object) => (
              <button
                key={`${object.objectType}-${object.name}`}
                className={
                  selectedObject?.name === object.name ? "object active" : "object"
                }
                title={object.status}
                onContextMenu={(event) => {
                  event.preventDefault();
                  setMenu({ object, x: event.clientX, y: event.clientY });
                }}
                onClick={() => void openObject(object)}
              >
                {object.name}
                <span>{object.objectType}</span>
              </button>
            ))}
          </div>
        </details>
        <button onClick={() => void checkUpdates()}>{t("checkUpdates")}</button>
      </aside>

      <main className="main">
        <header className="toolbar">
          <button onClick={() => void run()} disabled={!isConnected || running}>
            {t("execute")}
          </button>
          <button onClick={() => void cancel()} disabled={!running}>
            {t("cancel")}
          </button>
          <button onClick={() => void commit()} disabled={!isConnected}>
            {t("commit")}
          </button>
          <button onClick={() => void rollback()} disabled={!isConnected}>
            {t("rollback")}
          </button>
          <button onClick={() => void showHistory()}>{t("history")}</button>
          <button onClick={() => void explain()} disabled={!isConnected}>
            {t("explain")}
          </button>
          <button onClick={() => void showStats()} disabled={!isConnected}>
            {t("stats")}
          </button>
          <button onClick={() => void formatSql()} disabled={!sql.trim()}>
            {t("format")}
          </button>
          <button onClick={() => setPaletteOpen(true)}>{t("commandPalette")}</button>
          <button onClick={() => void saveWorkspace()}>{t("workspaceSave")}</button>
          <button onClick={() => void loadWorkspace()}>{t("workspaceLoad")}</button>
          <button onClick={() => void exportCsv()} disabled={!columns.length && !isConnected}>
            {t("exportCsv")}
          </button>
          <span className="connection-state">
            {isConnected ? `${t("connected")}: ${connected[selected]}` : t("noConnection")}
          </span>
        </header>

        <Editor
          ref={editorRef}
          initialValue={sql}
          tables={tables}
          loadColumns={loadColumnsFor}
          onChange={setSql}
          onExecute={(text) => void run(text)}
        />

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
          <span className="hint">{t("bookmarkHint")}</span>
        </nav>

        <section className="content">
          {panel === "results" && (
            <div
              className="results"
              onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
            >
              <table>
                <thead>
                  <tr>
                    {columns.map((column) => (
                      <th key={column.name} title={column.typeName}>
                        {column.name}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {start > 0 && (
                    <tr className="spacer" style={{ height: start * ROW_HEIGHT }}>
                      <td colSpan={Math.max(columns.length, 1)} />
                    </tr>
                  )}
                  {rows.slice(start, end).map((row, index) => (
                    <tr key={start + index} style={{ height: ROW_HEIGHT }}>
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
                      <td colSpan={Math.max(columns.length, 1)} />
                    </tr>
                  )}
                </tbody>
              </table>
              <p className="hint">
                {rows.length} {t("rows")}
              </p>
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
                  <span>{new Date(entry.startedAt * 1000).toLocaleString()}</span>
                  <code>{entry.sql}</code>
                  <span>
                    {entry.rows} {t("rows")} · {entry.elapsedMs} ms
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
                    <button onClick={() => void loadDdlFor(selectedObject)}>
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
                      <table>
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
                    <table>
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
                    <table>
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
                    <table>
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
                    <table>
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

          {panel === "stats" && (
            <table>
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

        <footer className="status">{status}</footer>
      </main>

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
                  <span>{command.name || command.id}</span>
                  <span className="keys">{command.keys[0] ?? ""}</span>
                  {!command.implemented && <em>{t("notImplemented")}</em>}
                </button>
              ))}
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
