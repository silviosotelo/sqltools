import { Icon } from "../icons";
import { useTranslation } from "../i18n";
import { CommandPalette, type PaletteProps } from "./CommandPalette";
import { EnvBadge } from "./EnvBadge";

export type ThemeMode = "light" | "dark" | "system";

const THEME_ICON = { light: "sun", dark: "moon", system: "themeAuto" } as const;

export function TitleBar({
  isConnected,
  connLabel,
  connTitle,
  theme,
  onTheme,
  language,
  onLanguage,
  palette,
  onChip,
  environment,
}: {
  isConnected: boolean;
  /** `user@alias` of the active connection. */
  connLabel: string;
  /** Tooltip with the server banner. */
  connTitle: string;
  theme: ThemeMode;
  onTheme: () => void;
  language: "es" | "en";
  onLanguage: (language: "es" | "en") => void;
  palette: PaletteProps;
  onChip: () => void;
  environment?: { label: string; color: string } | null;
}) {
  const { t } = useTranslation();
  return (
    <div className="titlebar">
      <h1 className="sr-only">
        {t("app.title")}, {t("app.subtitle")}
      </h1>
      <div className="brand">
        <Icon name="database" size={18} />
        <strong>{t("app.title")}</strong>
        <span className="brand-sub">{t("app.subtitle")}</span>
      </div>
      <CommandPalette {...palette} />
      <div className="titlebar-right">
        <button
          className="chip"
          data-e2e="conn-chip"
          title={connTitle}
          onClick={onChip}
        >
          <span className={isConnected ? "dot ok" : "dot"} />
          {isConnected ? (
            <>
              <span className="sr-only">{t("connected")}: </span>
              {connLabel}
              <EnvBadge environment={environment} />
            </>
          ) : (
            t("noConnection")
          )}
        </button>
        <button
          className="tb"
          title={t("theme")}
          aria-label={t("theme")}
          onClick={onTheme}
        >
          <Icon name={THEME_ICON[theme]} />
        </button>
        <select
          className="language-select"
          value={language}
          aria-label={t("app.language")}
          onChange={(event) => onLanguage(event.target.value as "es" | "en")}
        >
          <option value="es">Español</option>
          <option value="en">English</option>
        </select>
      </div>
    </div>
  );
}
