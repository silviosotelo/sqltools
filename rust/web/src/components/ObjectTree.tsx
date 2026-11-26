import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Icon, type IconName } from "../icons";
import { useTranslation } from "../i18n";
import type { ObjectInfo } from "../types";
import {
  nextFocus,
  validFocus,
  visibleWindow,
  type TreeKey,
  type TreeRow,
} from "../tree/treeModel";

const ROW_HEIGHT = 26;
const OVERSCAN = 6;
const KEYS: TreeKey[] = ["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End"];

const TYPE_ICON: Record<string, IconName> = {
  TABLE: "table",
  VIEW: "view",
  PACKAGE: "package",
  PROCEDURE: "procedure",
  FUNCTION: "function",
  TRIGGER: "trigger",
  SEQUENCE: "sequence",
  TYPE: "type",
  SYNONYM: "synonym",
};

export interface ObjectTreeProps {
  rows: TreeRow[];
  label: string;
  onToggle: (type: string) => void;
  onExpand: (type: string) => void;
  onRetry: (type: string) => void;
  onOpen: (object: ObjectInfo) => void;
  onContext: (object: ObjectInfo, x: number, y: number) => void;
  onGroupFocus: (type: string) => void;
}

/** Virtualised, keyboard-driven tree of catalog objects (WAI-ARIA tree with aria-activedescendant). */
export function ObjectTree({
  rows,
  label,
  onToggle,
  onExpand,
  onRetry,
  onOpen,
  onContext,
  onGroupFocus,
}: ObjectTreeProps) {
  const { t } = useTranslation();
  const box = useRef<HTMLDivElement | null>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewport, setViewport] = useState(400);
  const [activeId, setActiveId] = useState<string | undefined>();

  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    setViewport(el.clientHeight || 400);
    const observer = new ResizeObserver(() => setViewport(el.clientHeight || 400));
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  // The active row must exist: when a filter or a collapse removes it, fall back to a valid one.
  const active = validFocus(rows, activeId);
  useEffect(() => {
    if (active !== activeId) setActiveId(active);
  }, [active, activeId]);

  const scrollTo = (id: string) => {
    const index = rows.findIndex((row) => row.id === id);
    const el = box.current;
    if (index < 0 || !el) return;
    const top = index * ROW_HEIGHT;
    if (top < el.scrollTop) el.scrollTop = top;
    else if (top + ROW_HEIGHT > el.scrollTop + el.clientHeight) el.scrollTop = top + ROW_HEIGHT - el.clientHeight;
    setScrollTop(el.scrollTop);
  };

  const activate = (row: TreeRow) => {
    if (row.kind === "group") {
      if (row.status === "error") onRetry(row.objectType);
      else onToggle(row.objectType);
    } else if (row.object) onOpen(row.object);
  };

  const onKeyDown = (event: React.KeyboardEvent) => {
    const row = rows.find((r) => r.id === active);
    if (!row) return;
    if ((KEYS as string[]).includes(event.key)) {
      event.preventDefault();
      const move = nextFocus(rows, row.id, event.key as TreeKey);
      if (move.toggle === "expand") onExpand(row.objectType);
      if (move.toggle === "collapse") onToggle(row.objectType);
      setActiveId(move.focusId);
      scrollTo(move.focusId);
      const target = rows.find((r) => r.id === move.focusId);
      if (target?.kind === "group") onGroupFocus(target.objectType);
    } else if (event.key === "Enter") {
      event.preventDefault();
      activate(row);
    } else if ((event.key === "ContextMenu" || (event.shiftKey && event.key === "F10")) && row.object) {
      event.preventDefault();
      const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
      onContext(row.object, rect.left + 24, rect.top + 24);
    }
  };

  const { start, end } = visibleWindow(rows.length, scrollTop, viewport, ROW_HEIGHT, OVERSCAN);
  const idOf = (row: TreeRow) => `tree-${row.id.replace(/[^A-Za-z0-9_-]/g, "_")}-${row.id.length}`;
  const activeRow = rows.find((r) => r.id === active);
  // The active row stays mounted even when scrolled out of the window, so aria-activedescendant,
  // Enter and the context menu always refer to an element that exists.
  const activeIndex = activeRow ? rows.indexOf(activeRow) : -1;
  const renderIndexes: number[] = [];
  for (let i = start; i < end; i++) renderIndexes.push(i);
  if (activeIndex >= 0 && (activeIndex < start || activeIndex >= end)) renderIndexes.push(activeIndex);

  return (
    <div
      ref={box}
      className="tree"
      role="tree"
      tabIndex={0}
      aria-label={label}
      aria-activedescendant={activeRow ? idOf(activeRow) : undefined}
      onKeyDown={onKeyDown}
      onScroll={(event) => setScrollTop(event.currentTarget.scrollTop)}
    >
      <div className="tree-space" style={{ height: rows.length * ROW_HEIGHT }}>
        {renderIndexes.map((index) => {
          const row = rows[index];
          const isGroup = row.kind === "group";
          const type = row.objectType;
          return (
            <div
              key={row.id}
              id={idOf(row)}
              role="treeitem"
              data-type={type}
              aria-level={row.depth + 1}
              aria-setsize={row.setSize}
              aria-posinset={row.posInSet}
              aria-expanded={isGroup ? Boolean(row.expanded) : undefined}
              aria-selected={isGroup ? undefined : Boolean(row.selected)}
              className={`tree-row${isGroup ? " group" : ""}${row.id === active ? " active" : ""}${row.selected ? " selected" : ""}`}
              style={{ top: index * ROW_HEIGHT, height: ROW_HEIGHT, paddingLeft: 6 + row.depth * 16 }}
              title={row.object?.status}
              onClick={() => {
                setActiveId(row.id);
                box.current?.focus();
                if (isGroup) onGroupFocus(type);
                activate(row);
              }}
              onContextMenu={(event) => {
                if (!row.object) return;
                event.preventDefault();
                onContext(row.object, event.clientX, event.clientY);
              }}
            >
              {isGroup ? (
                <span className={row.expanded ? "tree-chevron open" : "tree-chevron"} aria-hidden="true">
                  <Icon name="chevron" size={12} />
                </span>
              ) : (
                <span className="tree-chevron" aria-hidden="true" />
              )}
              <span className="tree-icon" aria-hidden="true">
                <Icon name={TYPE_ICON[type] ?? "table"} size={14} />
              </span>
              <span className="tree-label">{row.label}</span>
              {isGroup && row.status === "loading" && <span className="tree-note">{t("loading")}</span>}
              {isGroup && row.status === "error" && <span className="tree-note error">{t("loadFailedRetry")}</span>}
              {isGroup && row.count !== undefined && row.status !== "loading" && row.status !== "error" && (
                <span className="tree-count">{row.count}</span>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}