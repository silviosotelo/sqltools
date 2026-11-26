import { useEffect, useRef } from "react";
import type { KeyboardEvent } from "react";
import { Icon } from "../icons";
import { useTranslation } from "../i18n";
import type { CommandSpec } from "../types";

export interface PaletteProps {
  open: boolean;
  query: string;
  index: number;
  commands: CommandSpec[];
  formatKeys: (keys: string[]) => string;
  onOpen: () => void;
  onClose: () => void;
  onQuery: (query: string) => void;
  onKeyDown: (event: KeyboardEvent<HTMLInputElement>) => void;
  onRun: (name: string) => void;
}

const optionId = (id: string) => `cmd-${id.replace(/[^A-Za-z0-9_-]/g, "_")}`;

/** Command palette anchored in the title bar: a WAI-ARIA combobox with a listbox popup. */
export function CommandPalette({
  open,
  query,
  index,
  commands,
  formatKeys,
  onOpen,
  onClose,
  onQuery,
  onKeyDown,
  onRun,
}: PaletteProps) {
  const { t } = useTranslation();
  const list = useRef<HTMLDivElement | null>(null);
  const active = commands[index];

  useEffect(() => {
    if (!open || !active) return;
    list.current?.querySelector(`#${optionId(active.id)}`)?.scrollIntoView({ block: "nearest" });
  }, [open, active]);

  if (!open) {
    return (
      <button className="palette-btn" onClick={onOpen}>
        <Icon name="search" />
        <span>{t("searchCommands")}</span>
        <kbd>Ctrl+Shift+P</kbd>
      </button>
    );
  }
  return (
    <div className="palette-anchor">
      <div className="palette-field">
        <Icon name="search" />
        <input
          autoFocus
          role="combobox"
          aria-expanded="true"
          aria-controls={commands.length ? "palette-listbox" : undefined}
          aria-autocomplete="list"
          aria-activedescendant={active ? optionId(active.id) : undefined}
          aria-label={t("searchCommands")}
          value={query}
          placeholder={t("searchCommands")}
          onChange={(event) => onQuery(event.target.value)}
          onKeyDown={onKeyDown}
          onBlur={onClose}
        />
      </div>
      {/* The popup scrolls, and axe requires a scrollable region to be reachable; the options themselves are
          not tab stops (focus stays in the input), and tabbing out of the input closes the palette. */}
      <div className="palette-pop" ref={list} tabIndex={0}>
        {commands.length > 0 && (
        <div id="palette-listbox" role="listbox" aria-label={t("commandPalette")}>
        {commands.map((command, i) => (
          <div
            key={command.id}
            id={optionId(command.id)}
            role="option"
            aria-selected={i === index}
            className={i === index ? "palette-option active" : "palette-option"}
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => onRun(command.name)}
          >
            <span>{command.name}</span>
            <span className="keys">{formatKeys(command.keys)}</span>
            {!command.implemented && <em>{t("notImplemented")}</em>}
          </div>
        ))}
        </div>
        )}
        {commands.length === 0 && (
          <div className="palette-empty" role="status">
            {t("noMatches")}
          </div>
        )}
      </div>
    </div>
  );
}