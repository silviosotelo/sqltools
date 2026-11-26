import type { Dispatch, RefObject, SetStateAction } from "react";
import { Icon } from "../icons";
import { useTranslation } from "../i18n";
import { OVERSCAN, ROW_HEIGHT, VISIBLE_ROWS, cellText, type ObjectTab, type Panel } from "../results";
import type {
  ColumnInfo,
  ColumnMeta,
  ConstraintInfo,
  Dependency,
  GrantInfo,
  IndexInfo,
  ObjectInfo,
  TriggerInfo,
  Value,
} from "../types";

export interface ResultsPanelProps {
  panel: Panel;
  setPanel: Dispatch<SetStateAction<Panel>>;
  columns: ColumnMeta[];
  rows: Value[][];
  scrollTop: number;
  setScrollTop: Dispatch<SetStateAction<number>>;
  setRows: Dispatch<SetStateAction<Value[][]>>;
  setColumns: Dispatch<SetStateAction<ColumnMeta[]>>;
  running: boolean;
  isConnected: boolean;
  dataPanelRef: RefObject<HTMLElement | null>;
  exportCsv: () => void | Promise<void>;
  setStatus: Dispatch<SetStateAction<string>>;
  setActiveCell: Dispatch<SetStateAction<{ column: string; value: string } | null>>;
  setCellView: Dispatch<SetStateAction<{ column: string; value: string } | null>>;
  plan: string;
  output: string[];
  selectedObject: ObjectInfo | null;
  objectTab: ObjectTab;
  loadDetail: (tab: ObjectTab) => void | Promise<void>;
  loadDdlFor: (object: ObjectInfo) => void | Promise<void>;
  objectColumns: ColumnInfo[];
  constraints: ConstraintInfo[];
  indexes: IndexInfo[];
  triggers: TriggerInfo[];
  grants: GrantInfo[];
  dependencies: Dependency[];
  onRecompile: (object: ObjectInfo) => void;
  onToggleTrigger: (trigger: TriggerInfo) => void;
  onToggleConstraint: (constraint: ConstraintInfo) => void;
}

/** The bottom panel: tab bar plus the content of the active tab (results grid, history, plan, object, output). */
export function ResultsPanel({
  panel, setPanel, columns, rows, scrollTop, setScrollTop, setRows, setColumns, running, isConnected, dataPanelRef, exportCsv, setStatus, setActiveCell, setCellView, plan, output, selectedObject, objectTab, loadDetail, loadDdlFor, objectColumns, constraints, indexes, triggers, grants, dependencies, onRecompile, onToggleTrigger, onToggleConstraint,
}: ResultsPanelProps) {
  const { t } = useTranslation();
  const start = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN);
  const end = Math.min(rows.length, start + VISIBLE_ROWS + OVERSCAN * 2);
  return (
    <>
      <div className="panel-bar">
        <nav className="tabs">
          <button
            className={panel === "results" ? "active" : ""}
            onClick={() => setPanel("results")}
          >
            {t("results")}
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

      <section className="content" ref={dataPanelRef} tabIndex={-1}>
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
                {!running && rows.length === 0 && (
                  <tr className="empty">
                    <td colSpan={Math.max(columns.length + 1, 1)}>
                      {t("emptyRows")}
                    </td>
                  </tr>
                )}
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
                        onClick={() =>
                          setActiveCell({
                            column: columns[columnIndex]?.name ?? "",
                            value: cellText(value),
                          })
                        }
                        onDoubleClick={() =>
                          setCellView({
                            column: columns[columnIndex]?.name ?? "",
                            value: cellText(value),
                          })
                        }
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
                  <button onClick={() => onRecompile(selectedObject)}>
                    {t("recompile")}
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
                      "dependencies",
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
                          <td>
                            {constraint.status}{" "}
                            <button
                              className="link-btn"
                              onClick={() => onToggleConstraint(constraint)}
                            >
                              {/^ENABLED$/.test(constraint.status)
                                ? t("disable")
                                : t("enable")}
                            </button>
                          </td>
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
                          <td>
                            {trigger.status}{" "}
                            <button
                              className="link-btn"
                              onClick={() => onToggleTrigger(trigger)}
                            >
                              {/^ENABLED$/.test(trigger.status)
                                ? t("disable")
                                : t("enable")}
                            </button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
                {objectTab === "dependencies" && (
                  <table className="grid">
                    <thead>
                      <tr>
                        <th>{t("dependency")}</th>
                        <th>{t("objectType")}</th>
                        <th>{t("name")}</th>
                      </tr>
                    </thead>
                    <tbody>
                      {dependencies.map((dependency, index) => (
                        <tr key={`${dependency.owner}-${dependency.name}-${index}`}>
                          <td>
                            {dependency.direction === "dependents"
                              ? t("dependents")
                              : t("referenced")}
                          </td>
                          <td>{dependency.objectType}</td>
                          <td>
                            {dependency.owner}.{dependency.name}
                          </td>
                        </tr>
                      ))}
                      {dependencies.length === 0 && (
                        <tr>
                          <td colSpan={3}>{t("noDependencies")}</td>
                        </tr>
                      )}
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
      </section>
    </>
  );
}
