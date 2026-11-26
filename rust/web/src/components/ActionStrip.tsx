import { useEffect, useRef, useState } from "react";
import { Icon, type IconName } from "../icons";
import { useTranslation } from "../i18n";
import { Breadcrumbs } from "./Breadcrumbs";

export interface MoreItem {
  id: string;
  label: string;
  onClick: () => void;
  disabled?: boolean;
}

export interface ActionStripProps {
  isConnected: boolean;
  hasSelection: boolean;
  running: boolean;
  hasSql: boolean;
  readOnly: boolean;
  transactionPending: boolean | null;
  onReadOnly: (value: boolean) => void;
  onRun: () => void;
  onCancel: () => void;
  onFormat: () => void;
  onExplain: () => void;
  onCommit: () => void;
  onRollback: () => void;
  onConnectToggle: () => void;
  more: MoreItem[];
  /** Shows a dot on the "more" button (an update is available). */
  moreAlert: boolean;
  breadcrumb: string[];
}

function Action({
  icon,
  label,
  onClick,
  disabled,
  mark,
}: {
  icon: IconName;
  label: string;
  onClick: () => void;
  disabled?: boolean;
  mark?: boolean;
}) {
  return (
    <button
      className={mark ? "tb marked" : "tb"}
      title={label}
      aria-label={label}
      onClick={onClick}
      disabled={disabled}
    >
      <Icon name={icon} />
    </button>
  );
}

/** Editor actions (run, format, plan, commit, rollback, connect) plus an overflow menu and the breadcrumbs. */
export function ActionStrip(props: ActionStripProps) {
  const { t } = useTranslation();
  const [menuOpen, setMenuOpen] = useState(false);
  const menu = useRef<HTMLDivElement | null>(null);
  const trigger = useRef<HTMLButtonElement | null>(null);
  const { isConnected, running } = props;

  useEffect(() => {
    if (menuOpen) menu.current?.querySelector<HTMLElement>("[role=menuitem]:not([aria-disabled='true'])")?.focus();
  }, [menuOpen]);

  const close = (restoreFocus: boolean) => {
    setMenuOpen(false);
    if (restoreFocus) trigger.current?.focus();
  };

  const onMenuKey = (event: React.KeyboardEvent) => {
    const items = Array.from(menu.current?.querySelectorAll<HTMLElement>("[role=menuitem]") ?? []);
    const at = items.indexOf(document.activeElement as HTMLElement);
    if (event.key === "Escape") {
      event.preventDefault();
      close(true);
    } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const step = event.key === "ArrowDown" ? 1 : -1;
      items[(at + step + items.length) % items.length]?.focus();
    } else if (event.key === "Home") {
      event.preventDefault();
      items[0]?.focus();
    } else if (event.key === "End") {
      event.preventDefault();
      items[items.length - 1]?.focus();
    } else if (event.key === "Tab") {
      close(false);
    }
  };

  return (
    <div className="actionstrip" role="group" aria-label={t("toolbar")}>
      <Action icon="run" label={`${t("execute")} (F5)`} onClick={props.onRun} disabled={!isConnected || running} />
      <Action icon="stop" label={t("cancel")} onClick={props.onCancel} disabled={!running} />
      <span className="tb-sep" />
      <Action icon="format" label={t("format")} onClick={props.onFormat} disabled={!props.hasSql} />
      <Action icon="plan" label={t("explain")} onClick={props.onExplain} disabled={!isConnected} />
      <span className="tb-sep" />
      <Action icon="commit" label={t("commit")} onClick={props.onCommit} disabled={!isConnected || running} mark={props.transactionPending === true} />
      <Action icon="rollback" label={t("rollback")} onClick={props.onRollback} disabled={!isConnected || running} mark={props.transactionPending === true} />
      <span className="tb-sep" />
      <label className="check-inline">
        <input type="checkbox" checked={props.readOnly} onChange={(event) => props.onReadOnly(event.target.checked)} />
        {t("readOnly")}
      </label>
      <span className="tb-sep" />
      <Action
        icon={isConnected ? "disconnect" : "connect"}
        label={isConnected ? t("disconnect") : t("connect")}
        onClick={props.onConnectToggle}
        disabled={!isConnected && !props.hasSelection}
      />
      <div className="more-wrap">
        <button
          ref={trigger}
          className={props.moreAlert ? "tb marked" : "tb"}
          title={t("more")}
          aria-label={t("more")}
          aria-haspopup="menu"
          aria-expanded={menuOpen}
          aria-controls={menuOpen ? "more-menu" : undefined}
          onClick={() => setMenuOpen((open) => !open)}
        >
          <Icon name="more" />
        </button>
        {menuOpen && (
          <div
            id="more-menu"
            ref={menu}
            role="menu"
            aria-label={t("more")}
            className="more-menu"
            onKeyDown={onMenuKey}
            onBlur={(event) => {
              if (!event.currentTarget.contains(event.relatedTarget as Node | null) && event.relatedTarget !== trigger.current) setMenuOpen(false);
            }}
          >
            {props.more.map((item) => (
              <button
                key={item.id}
                role="menuitem"
                tabIndex={-1}
                aria-disabled={item.disabled || undefined}
                onClick={() => {
                  if (item.disabled) return;
                  close(false);
                  item.onClick();
                  // Same rule as the palette: never leave the focus on <body> when the menu unmounts.
                  window.setTimeout(() => {
                    if (!document.activeElement || document.activeElement === document.body) {
                      trigger.current?.focus();
                    }
                  }, 0);
                }}
              >
                {item.label}
              </button>
            ))}
          </div>
        )}
      </div>
      <Breadcrumbs parts={props.breadcrumb} />
    </div>
  );
}