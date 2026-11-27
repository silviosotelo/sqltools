import { Icon, type IconName } from "../icons";
import { useTranslation, type MessageKey } from "../i18n";

export type SideView =
  | "objects"
  | "connections"
  | "history"
  | "stats"
  | "todos"
  | "files";

const VIEWS: Array<{ id: SideView; icon: IconName; label: MessageKey }> = [
  { id: "objects", icon: "database", label: "objects" },
  { id: "connections", icon: "connect", label: "connections" },
  { id: "history", icon: "history", label: "history" },
  { id: "stats", icon: "stats", label: "stats" },
  { id: "todos", icon: "warning", label: "todos" },
  { id: "files", icon: "open", label: "files" },
];

export function ActivityBar({
  view,
  onView,
  onShortcuts,
  onSettings,
}: {
  view: SideView;
  onView: (view: SideView) => void;
  onShortcuts: () => void;
  onSettings: () => void;
}) {
  const { t } = useTranslation();
  return (
    <nav className="activity" aria-label={t("views")}>
      {VIEWS.map((item) => (
        <button
          key={item.id}
          className="act"
          aria-pressed={view === item.id}
          aria-label={t(item.label)}
          title={t(item.label)}
          onClick={() => onView(item.id)}
        >
          <Icon name={item.icon} size={20} />
        </button>
      ))}
      <span className="act-grow" />
      <button
        className="act"
        aria-label={t("shortcuts")}
        title={t("shortcuts")}
        onClick={onShortcuts}
      >
        <Icon name="keyboard" size={20} />
      </button>
      <button
        className="act"
        aria-label={t("permanentSettings")}
        title={t("permanentSettings")}
        onClick={onSettings}
      >
        <Icon name="settings" size={20} />
      </button>
    </nav>
  );
}
