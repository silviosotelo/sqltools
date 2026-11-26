import type { ObjectInfo } from "../types";

export type GroupStatus = "idle" | "loading" | "ready" | "error";

export interface GroupState {
  status: GroupStatus;
  /** Incremented on every load; a result carrying an older id is stale and ignored. */
  requestId: number;
  items: ObjectInfo[];
  error?: string;
}

export interface TreeGroup {
  type: string;
  label: string;
}

export interface TreeModel {
  groups: TreeGroup[];
  counts: Record<string, number>;
  states: Record<string, GroupState>;
  expanded: ReadonlySet<string>;
  filter: string;
  selectedId?: string;
}

export interface TreeRow {
  id: string;
  kind: "group" | "object";
  depth: number;
  label: string;
  objectType: string;
  count?: number;
  expanded?: boolean;
  status?: GroupStatus;
  selected?: boolean;
  /** Position among the siblings of the same level (1-based) and their number, for aria-posinset/setsize. */
  posInSet: number;
  setSize: number;
  parentId?: string;
  /** The catalog entry behind an `object` row. */
  object?: ObjectInfo;
}

export const groupId = (type: string) => `g:${type}`;
export const objectId = (type: string, name: string) => `o:${type}:${name}`;

export const idleGroup = (): GroupState => ({ status: "idle", requestId: 0, items: [] });

export function startLoad(state: GroupState | undefined): GroupState {
  const current = state ?? idleGroup();
  return { ...current, status: "loading", requestId: current.requestId + 1, error: undefined };
}

export type LoadResult = { ok: true; items: ObjectInfo[] } | { ok: false; error: string };

/** Applies a load result unless a newer load has been started since (stale responses are dropped). */
export function applyLoadResult(state: GroupState, requestId: number, result: LoadResult): GroupState {
  if (requestId !== state.requestId) return state;
  return result.ok
    ? { ...state, status: "ready", items: result.items, error: undefined }
    : { ...state, status: "error", error: result.error };
}

const matches = (name: string, filter: string) =>
  filter === "" || name.toLowerCase().includes(filter.toLowerCase());

/** Flattens the model into the rows that are visible (children of collapsed groups are hidden). */
export function flattenTree(model: TreeModel): TreeRow[] {
  const filter = model.filter.trim();
  const groups = model.groups.filter((group) => {
    if (filter === "") return true;
    const state = model.states[group.type];
    // With a filter, a loaded group without matches disappears; unloaded groups stay (they may match).
    return !state || state.status !== "ready" || state.items.some((o) => matches(o.name, filter));
  });
  const rows: TreeRow[] = [];
  groups.forEach((group, index) => {
    const id = groupId(group.type);
    const state = model.states[group.type];
    const expanded = model.expanded.has(group.type);
    rows.push({
      id,
      kind: "group",
      depth: 0,
      label: group.label,
      objectType: group.type,
      count: model.counts[group.type],
      expanded,
      status: state?.status ?? "idle",
      selected: model.selectedId === id,
      posInSet: index + 1,
      setSize: groups.length,
    });
    if (!expanded || !state || state.status !== "ready") return;
    const items = state.items.filter((o) => matches(o.name, filter));
    items.forEach((item, i) => {
      const oid = objectId(group.type, item.name);
      rows.push({
        id: oid,
        kind: "object",
        depth: 1,
        label: item.name,
        objectType: group.type,
        selected: model.selectedId === oid,
        posInSet: i + 1,
        setSize: items.length,
        parentId: id,
        object: item,
      });
    });
  });
  return rows;
}

/** The slice of rows to render: the viewport plus `overscan` rows on each side. */
export function visibleWindow(
  total: number,
  scrollTop: number,
  viewport: number,
  rowHeight: number,
  overscan: number,
): { start: number; end: number } {
  const first = Math.floor(Math.max(0, scrollTop) / rowHeight);
  const count = Math.ceil(viewport / rowHeight);
  const start = Math.max(0, first - overscan);
  const end = Math.min(total, first + count + overscan);
  return { start, end };
}

export type TreeKey = "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" | "Home" | "End";

/** Where keyboard focus goes for `key`, and whether the focused group must expand or collapse. */
export function nextFocus(
  rows: TreeRow[],
  focusedId: string,
  key: TreeKey,
): { focusId: string; toggle?: "expand" | "collapse" } {
  if (rows.length === 0) return { focusId: focusedId };
  const index = rows.findIndex((row) => row.id === focusedId);
  const at = index < 0 ? 0 : index;
  const row = rows[at];
  const clamp = (i: number) => rows[Math.min(rows.length - 1, Math.max(0, i))].id;
  switch (key) {
    case "ArrowDown":
      return { focusId: clamp(at + 1) };
    case "ArrowUp":
      return { focusId: clamp(at - 1) };
    case "Home":
      return { focusId: rows[0].id };
    case "End":
      return { focusId: rows[rows.length - 1].id };
    case "ArrowRight":
      if (row.kind === "group" && !row.expanded) return { focusId: row.id, toggle: "expand" };
      if (row.kind === "group" && rows[at + 1]?.parentId === row.id) return { focusId: rows[at + 1].id };
      return { focusId: row.id };
    case "ArrowLeft":
      if (row.kind === "group" && row.expanded) return { focusId: row.id, toggle: "collapse" };
      if (row.parentId) return { focusId: row.parentId };
      return { focusId: row.id };
  }
}

/** If the focused row is no longer visible (filter, collapse), the nearest row that still exists. */
export function validFocus(rows: TreeRow[], focusedId: string | undefined): string | undefined {
  if (rows.length === 0) return undefined;
  if (focusedId && rows.some((row) => row.id === focusedId)) return focusedId;
  return rows[0].id;
}