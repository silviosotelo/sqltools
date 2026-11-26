import { describe, expect, it } from "vitest";
import {
  applyLoadResult,
  flattenTree,
  groupId,
  idleGroup,
  nextFocus,
  objectId,
  startLoad,
  validFocus,
  visibleWindow,
  type TreeModel,
} from "./treeModel";

const obj = (name: string, objectType = "TABLE") => ({ name, objectType, status: "VALID" });
const model = (over: Partial<TreeModel> = {}): TreeModel => ({
  groups: [
    { type: "TABLE", label: "Tables" },
    { type: "PACKAGE", label: "Packages" },
  ],
  counts: { TABLE: 3, PACKAGE: 1 },
  states: {
    TABLE: { status: "ready", requestId: 1, items: [obj("A"), obj("B"), obj("C")] },
    PACKAGE: { status: "ready", requestId: 1, items: [obj("PK", "PACKAGE")] },
  },
  expanded: new Set(["TABLE"]),
  filter: "",
  ...over,
});

describe("flattenTree", () => {
  it("hides children of collapsed groups", () => {
    const rows = flattenTree(model());
    expect(rows.map((r) => r.id)).toEqual([
      groupId("TABLE"),
      objectId("TABLE", "A"),
      objectId("TABLE", "B"),
      objectId("TABLE", "C"),
      groupId("PACKAGE"),
    ]);
  });
  it("computes level, setsize and posinset over the filtered tree", () => {
    const rows = flattenTree(model({ filter: "b" }));
    const b = rows.find((r) => r.id === objectId("TABLE", "B"))!;
    expect([b.depth, b.posInSet, b.setSize]).toEqual([1, 1, 1]);
    expect(rows.find((r) => r.id === groupId("PACKAGE"))).toBeUndefined(); // loaded, no match
  });
  it("keeps unloaded groups visible while filtering", () => {
    const rows = flattenTree(model({ filter: "zzz", states: {} }));
    expect(rows.map((r) => r.kind)).toEqual(["group", "group"]);
  });
});

describe("visibleWindow", () => {
  it("never exceeds viewport/rowHeight + 2*overscan rows", () => {
    for (const top of [0, 13, 520, 99999]) {
      const { start, end } = visibleWindow(5000, top, 260, 26, 6);
      expect(end - start).toBeLessThanOrEqual(Math.ceil(260 / 26) + 12);
    }
  });
  it("clamps to the total", () => {
    expect(visibleWindow(10, 99999, 260, 26, 6).end).toBe(10);
    expect(visibleWindow(0, 0, 260, 26, 6)).toEqual({ start: 0, end: 0 });
  });
});

describe("nextFocus", () => {
  const rows = flattenTree(model());
  it("ArrowRight on a collapsed group expands, on an expanded group moves to its first child", () => {
    const collapsed = flattenTree(model({ expanded: new Set() }));
    expect(nextFocus(collapsed, groupId("TABLE"), "ArrowRight")).toEqual({
      focusId: groupId("TABLE"),
      toggle: "expand",
    });
    expect(nextFocus(rows, groupId("TABLE"), "ArrowRight").focusId).toBe(objectId("TABLE", "A"));
  });
  it("ArrowLeft on a child moves to its parent; on an expanded group collapses", () => {
    expect(nextFocus(rows, objectId("TABLE", "B"), "ArrowLeft").focusId).toBe(groupId("TABLE"));
    expect(nextFocus(rows, groupId("TABLE"), "ArrowLeft").toggle).toBe("collapse");
  });
  it("Home/End go to the first/last visible row and arrows clamp", () => {
    expect(nextFocus(rows, objectId("TABLE", "B"), "Home").focusId).toBe(rows[0].id);
    expect(nextFocus(rows, objectId("TABLE", "B"), "End").focusId).toBe(rows.at(-1)!.id);
    expect(nextFocus(rows, rows[0].id, "ArrowUp").focusId).toBe(rows[0].id);
    expect(nextFocus(rows, rows.at(-1)!.id, "ArrowDown").focusId).toBe(rows.at(-1)!.id);
  });
});

describe("loads", () => {
  it("a stale requestId result is ignored", () => {
    const first = startLoad(idleGroup());
    const second = startLoad(first);
    const afterStale = applyLoadResult(second, first.requestId, { ok: true, items: [obj("OLD")] });
    expect(afterStale).toBe(second);
    const done = applyLoadResult(second, second.requestId, { ok: true, items: [obj("NEW")] });
    expect(done.status).toBe("ready");
    expect(done.items[0].name).toBe("NEW");
  });
  it("a failure leaves the group in error, and a retry clears it", () => {
    const failed = applyLoadResult(startLoad(undefined), 1, { ok: false, error: "boom" });
    expect([failed.status, failed.error]).toEqual(["error", "boom"]);
    expect(startLoad(failed).error).toBeUndefined();
  });
});

describe("validFocus", () => {
  it("falls back to the first row when the focused row left the tree", () => {
    const rows = flattenTree(model());
    expect(validFocus(rows, objectId("TABLE", "GONE"))).toBe(rows[0].id);
    expect(validFocus(rows, objectId("TABLE", "B"))).toBe(objectId("TABLE", "B"));
    expect(validFocus([], "x")).toBeUndefined();
  });
});