import { useTranslation } from "../i18n";
import type { FileEntry } from "../types";

/** Side view: the declared locations and the folders inside them; a .sql file opens in the editor. */
export function FilesView({
  roots,
  dir,
  entries,
  onRoot,
  onUp,
  onEnter,
  onOpen,
  onAddLocation,
}: {
  roots: string[];
  dir: string;
  entries: FileEntry[];
  onRoot: (path: string) => void;
  onUp: () => void;
  onEnter: (path: string) => void;
  onOpen: (entry: FileEntry) => void;
  onAddLocation: () => void;
}) {
  const { t } = useTranslation();
  const short = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  return (
    <section className="side-section side-fill" aria-labelledby="files-title">
      <h2 id="files-title" className="panel-title">
        {t("files")}
      </h2>
      <div className="section-body">
        <div className="row">
          <button className="plain" onClick={onAddLocation}>
            {t("filesAddLocation")}
          </button>
          <button className="plain" onClick={onUp} disabled={!dir}>
            {t("filesUp")}
          </button>
        </div>
        {roots.length > 0 && (
          <ul className="conn-list" aria-label={t("filesRoots")}>
            {roots.map((root) => (
              <li key={root}>
                <button
                  className={root === dir ? "conn-item active" : "conn-item"}
                  title={root}
                  onClick={() => onRoot(root)}
                >
                  <span className="conn-name">{short(root)}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
        <ul className="conn-list" aria-label={t("files")}>
          {entries.map((entry) => (
            <li key={entry.path}>
              <button
                className="conn-item"
                title={entry.path}
                onClick={() => (entry.isDir ? onEnter(entry.path) : onOpen(entry))}
              >
                <span className="conn-name">{entry.isDir ? `${entry.name}/` : entry.name}</span>
              </button>
            </li>
          ))}
          {entries.length === 0 && dir !== "" && <li className="side-empty">{t("filesEmpty")}</li>}
        </ul>
      </div>
    </section>
  );
}
