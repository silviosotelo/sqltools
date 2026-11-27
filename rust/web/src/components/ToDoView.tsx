import { useTranslation } from "../i18n";
import type { TodoItem } from "../types";

/** Side view: the TODO/FIXME notes found in the schema sources, clickable to open the object. */
export function ToDoView({
  items,
  onOpen,
}: {
  items: TodoItem[];
  onOpen: (item: TodoItem) => void;
}) {
  const { t } = useTranslation();
  return (
    <section className="side-section side-fill" aria-labelledby="todos-title">
      <h2 id="todos-title" className="panel-title">
        {t("todos")}
      </h2>
      <div className="history" title={t("todoHint")}>
        {items.length === 0 && <p className="side-empty">{t("todoEmpty")}</p>}
        {items.map((item, index) => (
          <button
            key={`${item.owner}.${item.name}.${item.line}.${index}`}
            className="history-row"
            onClick={() => onOpen(item)}
          >
            <span>
              {item.marker} · {item.objectType} {item.name}:{item.line}
            </span>
            <code>{item.text}</code>
          </button>
        ))}
      </div>
    </section>
  );
}
