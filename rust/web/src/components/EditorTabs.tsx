import { Icon } from "../icons";
import { useTranslation } from "../i18n";

export interface TabDoc {
  id: string;
  title: string;
  path: string;
  dirty: boolean;
}

/** The strip of open documents above the editor. */
export function EditorTabs({
  docs,
  activeId,
  onSelect,
  onClose,
}: {
  docs: TabDoc[];
  activeId: string;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="editor-tabbar">
      <ul className="editor-tab-list" aria-label={t("openFiles")}>
        {docs.map((doc) => (
          <li
            key={doc.id}
            className={doc.id === activeId ? "editor-tab active" : "editor-tab"}
            title={doc.path || doc.title}
          >
            <button
              className="tab-btn"
              aria-current={doc.id === activeId ? "true" : undefined}
              onClick={() => onSelect(doc.id)}
            >
              <span className="tab-title">
                {doc.dirty ? "*" : ""}
                {doc.title}
              </span>
            </button>
            <button
              className="tab-close"
              title={t("close")}
              aria-label={`${t("close")}: ${doc.title}`}
              onClick={() => onClose(doc.id)}
            >
              <Icon name="close" size={12} />
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
