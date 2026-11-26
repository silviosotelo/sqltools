import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../api";
import type { ObjectInfo } from "../types";
import {
  applyLoadResult,
  flattenTree,
  idleGroup,
  startLoad,
  type GroupState,
  type TreeGroup,
  type TreeRow,
} from "./treeModel";

/**
 * Object tree state: counts at connect, one lazy `list_objects` per group on expand. Every response is
 * checked against the connection/schema epoch and the per-group request id, so late answers are dropped.
 */
export function useObjectTree(
  connectionId: string | null,
  schema: string,
  groups: TreeGroup[],
  selectedId?: string,
) {
  const [counts, setCounts] = useState<Record<string, number>>({});
  const [states, setStates] = useState<Record<string, GroupState>>({});
  const [expanded, setExpanded] = useState<ReadonlySet<string>>(new Set());
  const [filter, setFilter] = useState("");
  const [loadingAll, setLoadingAll] = useState(false);
  const epoch = useRef(0);
  const ids = useRef<Record<string, number>>({});
  const runId = useRef(0);
  const owner = schema.trim() || null;
  const latest = useRef({ connectionId, owner });
  latest.current = { connectionId, owner };

  const loadCounts = useCallback(() => {
    const mine = epoch.current;
    const { connectionId: id, owner: o } = latest.current;
    if (!id) return;
    api
      .countObjects(id, o)
      .then((list) => {
        if (mine !== epoch.current) return;
        setCounts(Object.fromEntries(list.map((c) => [c.objectType, c.count])));
      })
      .catch(() => undefined); // counts are decoration; a failure leaves the groups without a number
  }, []);

  // New connection or schema: invalidate everything in flight and start over, during render so no row of
  // the previous context is ever shown or accepted (the counts request alone is debounced).
  const contextKey = `${connectionId ?? ""}\u0000${owner ?? ""}`;
  const [shownContext, setShownContext] = useState(contextKey);
  if (shownContext !== contextKey) {
    setShownContext(contextKey);
    setCounts({});
    setStates({});
    setExpanded(new Set());
    setLoadingAll(false);
  }
  const knownContext = useRef(contextKey);
  if (knownContext.current !== contextKey) {
    knownContext.current = contextKey;
    epoch.current += 1;
    ids.current = {};
    runId.current += 1;
  }
  useEffect(() => {
    if (!connectionId) return;
    const timer = setTimeout(loadCounts, 400);
    return () => clearTimeout(timer);
  }, [contextKey, connectionId, loadCounts]);

  const loadGroup = useCallback(async (type: string) => {
    const { connectionId: id, owner: o } = latest.current;
    if (!id) return;
    const mine = epoch.current;
    const requestId = (ids.current[type] ?? 0) + 1;
    ids.current[type] = requestId;
    setStates((prev) => ({ ...prev, [type]: { ...startLoad(prev[type]), requestId } }));
    let result: { ok: true; items: ObjectInfo[] } | { ok: false; error: string };
    try {
      result = { ok: true, items: await api.listObjects(id, o, type) };
    } catch (error) {
      result = { ok: false, error: String(error) };
    }
    if (mine !== epoch.current || ids.current[type] !== requestId) return;
    setStates((prev) => ({
      ...prev,
      [type]: applyLoadResult({ ...(prev[type] ?? idleGroup()), requestId }, requestId, result),
    }));
  }, []);

  const toggle = useCallback(
    (type: string) => {
      const open = expanded.has(type);
      setExpanded((prev) => {
        const next = new Set(prev);
        if (open) next.delete(type);
        else next.add(type);
        return next;
      });
      const state = states[type];
      if (!open && (!state || state.status === "idle" || state.status === "error")) void loadGroup(type);
    },
    [expanded, states, loadGroup],
  );

  const expand = useCallback(
    (type: string) => {
      if (!expanded.has(type)) toggle(type);
    },
    [expanded, toggle],
  );

  /** Reloads the counts and every group that is currently expanded. */
  const refresh = useCallback(() => {
    loadCounts();
    for (const type of expanded) void loadGroup(type);
  }, [expanded, loadCounts, loadGroup]);

  /** Loads every group, one request at a time, so the filter can search the whole schema. */
  const loadAll = useCallback(async () => {
    runId.current += 1;
    const run = runId.current;
    const mine = epoch.current;
    const current = () => run === runId.current && mine === epoch.current;
    setLoadingAll(true);
    for (const group of groups) {
      if (!current()) return;
      if (states[group.type]?.status === "ready") continue;
      await loadGroup(group.type);
      if (!current()) return;
      setExpanded((prev) => new Set(prev).add(group.type));
    }
    if (current()) setLoadingAll(false);
  }, [groups, states, loadGroup]);

  const cancelLoadAll = useCallback(() => {
    runId.current += 1;
    setLoadingAll(false);
  }, []);

  const rows: TreeRow[] = useMemo(
    () => flattenTree({ groups, counts, states, expanded, filter, selectedId }),
    [groups, counts, states, expanded, filter, selectedId],
  );

  return { rows, toggle, expand, refresh, loadAll, cancelLoadAll, loadingAll, filter, setFilter, retry: loadGroup, states };
}