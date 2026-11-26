import { ENV_COLORS, ENV_LABEL_MAX, type EnvColor } from "../environment";
import { useTranslation } from "../i18n";
import type { ConnectionConfig } from "../types";
import { EnvBadge } from "./EnvBadge";

export interface ConnectionDraft {
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
  envLabel: string;
  envColor: EnvColor;
}

const COLOR_KEY = {
  neutral: "envNeutral",
  cyan: "envCyan",
  green: "envGreen",
  amber: "envAmber",
  red: "envRed",
  violet: "envViolet",
} as const;

export interface ConnectionsViewProps {
  connections: ConnectionConfig[];
  selected: string;
  connected: Record<string, string>;
  draft: ConnectionDraft;
  password: string;
  update: <K extends keyof ConnectionDraft>(key: K, value: ConnectionDraft[K]) => void;
  setPassword: (value: string) => void;
  onSelect: (id: string) => void;
  onNew: () => void;
  onSave: () => void;
  onConnect: () => void;
  onRemove: () => void;
  onImportLegacy: () => void;
}

/** Side view: saved connections (with their environment badge) and the form to create or edit one. */
export function ConnectionsView({
  connections,
  selected,
  connected,
  draft,
  password,
  update,
  setPassword,
  onSelect,
  onNew,
  onSave,
  onConnect,
  onRemove,
  onImportLegacy,
}: ConnectionsViewProps) {
  const { t } = useTranslation();
  return (
    <section className="side-section conn-view" aria-labelledby="connections-title">
      <h2 id="connections-title" className="panel-title">
        {t("connections")}
      </h2>
      <ul className="conn-list" aria-label={t("connections")}>
        {connections.map((config) => (
          <li key={config.id}>
            <button
              className={config.id === selected ? "conn-item active" : "conn-item"}
              aria-current={config.id === selected ? "true" : undefined}
              onClick={() => onSelect(config.id)}
            >
              <span className={connected[config.id] ? "dot ok" : "dot"} aria-hidden="true" />
              <span className="conn-name">{config.label}</span>
              <EnvBadge environment={config.environment} />
              {connected[config.id] && <span className="sr-only">{t("connected")}</span>}
            </button>
          </li>
        ))}
      </ul>
      <div className="section-body">
        <label className="field-row">
          <span>{t("label")}</span>
          <input value={draft.label} onChange={(event) => update("label", event.target.value)} />
        </label>
        <label className="field-row">
          <span>{t("user")}</span>
          <input value={draft.user} onChange={(event) => update("user", event.target.value)} />
        </label>
        <label className="field-row">
          <span>{t("password")}</span>
          <input type="password" value={password} onChange={(event) => setPassword(event.target.value)} />
        </label>
        <label className="field-row">
          <span>{t("alias")}</span>
          <input value={draft.alias} onChange={(event) => update("alias", event.target.value)} />
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
          <input value={draft.tnsAdmin} onChange={(event) => update("tnsAdmin", event.target.value)} />
        </label>
        <label className="field-row">
          <span>{t("ociDir")}</span>
          <input value={draft.ociDir} onChange={(event) => update("ociDir", event.target.value)} />
        </label>
        <label className="field-row">
          <span>{t("mode")}</span>
          <select
            value={draft.mode}
            onChange={(event) => update("mode", event.target.value as ConnectionDraft["mode"])}
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
            onChange={(event) => update("prefetch", Number(event.target.value) || 1000)}
          />
        </label>
        <label className="check-inline">
          <input
            type="checkbox"
            checked={draft.savePassword}
            onChange={(event) => update("savePassword", event.target.checked)}
          />
          {t("savePassword")}
        </label>
        <fieldset className="env-field">
          <legend>{t("environment")}</legend>
          <label className="field-row">
            <span>{t("envLabel")}</span>
            <input
              value={draft.envLabel}
              maxLength={ENV_LABEL_MAX * 2}
              placeholder={t("envLabelHint")}
              onChange={(event) => update("envLabel", event.target.value)}
            />
          </label>
          <div
            role="radiogroup"
            aria-label={t("envColor")}
            className="env-swatches"
            onKeyDown={(event) => {
              const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];
              if (!step) return;
              event.preventDefault();
              const at = ENV_COLORS.indexOf(draft.envColor);
              const next = ENV_COLORS[(at + step + ENV_COLORS.length) % ENV_COLORS.length];
              update("envColor", next);
              const group = event.currentTarget;
              requestAnimationFrame(() =>
                group.querySelector<HTMLElement>(`[data-color="${next}"]`)?.focus(),
              );
            }}
          >
            {ENV_COLORS.map((color) => (
              <button
                key={color}
                type="button"
                role="radio"
                aria-checked={draft.envColor === color}
                tabIndex={draft.envColor === color ? 0 : -1}
                aria-label={t(COLOR_KEY[color])}
                title={t(COLOR_KEY[color])}
                className={draft.envColor === color ? "swatch active" : "swatch"}
                data-color={color}
                onClick={() => update("envColor", color)}
              />
            ))}
          </div>
          {draft.envLabel.trim() !== "" && (
            <div className="env-preview" aria-hidden="true">
              <EnvBadge environment={{ label: draft.envLabel.trim().slice(0, ENV_LABEL_MAX), color: draft.envColor }} />
            </div>
          )}
        </fieldset>
        <div className="row">
          <button className="plain" onClick={onNew}>
            {t("newConnection")}
          </button>
          <button className="plain" onClick={onSave}>
            {t("save")}
          </button>
          <button className="plain" onClick={onConnect} disabled={!selected}>
            {t("connect")}
          </button>
          <button className="plain" onClick={onRemove} disabled={!selected}>
            {t("delete")}
          </button>
          <button className="plain" onClick={onImportLegacy}>
            {t("importLegacy")}
          </button>
        </div>
      </div>
    </section>
  );
}