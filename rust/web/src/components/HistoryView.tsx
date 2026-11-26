import { useTranslation } from "../i18n";
import type { HistoryEntry } from "../types";

/** Side view: the executed statements of the session; clicking one puts it back into the editor. */
export function HistoryView({
  entries,
  onPick,
}: {
  entries: HistoryEntry[];
  onPick: (sql: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <section className="side-section side-fill" aria-labelledby="history-title">
      <h2 id="history-title" className="panel-title">
        {t("history")}
      </h2>
      <div className="history">
        {entries.length === 0 && <p className="side-empty">{t("historyEmpty")}</p>}
        {entries.map((entry, index) => (
          <button key={index} className="history-row" onClick={() => onPick(entry.sql)}>
            <span>{new Date(entry.startedAt * 1000).toLocaleString()}</span>
            <code>{entry.sql}</code>
            <span>
              {entry.rows} {entry.rows === 1 ? t("row") : t("rows")} · {entry.elapsedMs} ms
              {entry.error ? ` · ${entry.error}` : ""}
            </span>
          </button>
        ))}
      </div>
    </section>
  );
}