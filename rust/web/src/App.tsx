import { useEffect, useState } from "react";
import { Editor } from "./Editor";
import { api } from "./api";
import { useTranslation } from "./i18n";
import type {
  ColumnInfo,
  ColumnMeta,
  ConnectionConfig,
  HistoryEntry,
  ObjectInfo,
  QueryEvent,
  StatEntry,
  Value,
} from "./types";

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
  const [stats, setStats] = useState<StatEntry[]>([]);

  const isConnected = Boolean(selected && connected[selected]);

  const refresh = async () => {
    setConnections(await api.listConnections());
  };

  useEffect(() => {
    void refresh();
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
    setStats([]);
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

  const openObject = async (object: ObjectInfo) => {
    setSelectedObject(object);
    setObjectColumns([]);
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

  const loadDdl = async () => {
    if (!selectedObject) return;
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

  const exportCsv = () => {
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
                onClick={() => void openObject(object)}
              >
                {object.name}
                <span>{object.objectType}</span>
              </button>
            ))}
          </div>
        </details>
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
          <button onClick={exportCsv} disabled={!columns.length}>
            {t("exportCsv")}
          </button>
          <span className="connection-state">
            {isConnected ? `${t("connected")}: ${connected[selected]}` : t("noConnection")}
          </span>
        </header>

        <Editor
          initialValue={sql}
          tables={tables}
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
                    <button onClick={() => void loadDdl()}>{t("loadDdl")}</button>
                  </header>
                  {objectColumns.length > 0 ? (
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
    </div>
  );
}
