import { describe, expect, it } from "vitest";
import { ROWID_COLUMN, prepareEditable, queryByExample } from "./editable";

describe("prepareEditable", () => {
  it("adds the hidden ROWID to a single-table select", () => {
    const result = prepareEditable("select a, b from A11 where a = 1 order by b");
    expect(result).not.toBeNull();
    expect(result!.sql).toBe(
      `select a, b, t.ROWID as "${ROWID_COLUMN}" from A11 t where a = 1 order by b`,
    );
    expect(result!.table).toBe("A11");
  });
  it("qualifies * and accepts an owner prefix", () => {
    expect(prepareEditable("SELECT * FROM ANAMNESIS.A11")!.sql).toBe(
      `select t.*, t.ROWID as "${ROWID_COLUMN}" from ANAMNESIS.A11 t`,
    );
  });
  it("is case insensitive and ignores the trailing semicolon", () => {
    expect(prepareEditable("  SeLeCt * FrOm t;  ")!.table).toBe("t");
  });
  it("refuses everything that is not a single table", () => {
    for (const sql of [
      "select * from a join b on a.id = b.id",
      "select * from a, b",
      "select * from (select * from a)",
      "select count(*) from a",
      "select distinct a from t",
      "select * from t group by a",
      "select * from t for update",
      "select rowid from t",
      "select * from t@remote",
      "select * from t x where x.a = 1",
      "update t set a = 1",
      "select * from t where a = :1",
      "select * from t; select * from u",
      "select a as b from t",
      "select n name from t",
      "select a as b from t",
      "select n name from t",
    ]) {
      expect(prepareEditable(sql), sql).toBeNull();
    }
  });
  it("accepts a where clause with commas or strings", () => {
    expect(prepareEditable("select a from t where b in (1, 2) and c = 'x,y'")).not.toBeNull();
  });
  it("keeps the where clause byte for byte", () => {
    const result = prepareEditable("select a from t where c = 'a  b'");
    expect(result!.sql).toContain("where c = 'a  b'");
    const commented = prepareEditable("select a from t -- note\nwhere c = 1");
    expect(commented).toBeNull();
  });
});

describe("queryByExample", () => {
  it("wraps the query and keeps ORDER BY inside the inline view", () => {
    expect(queryByExample("select a, b from t order by a", "b > 2")).toBe(
      "select * from (select a, b from t order by a) where b > 2",
    );
    expect(queryByExample("select a from t order by t.a", "a > 1")).toContain("order by t.a)");
  });
  it("returns the query unchanged when the condition is empty", () => {
    expect(queryByExample("select * from t order by a", "  ")).toBe("select * from t order by a");
  });
  it("does not touch an ORDER BY inside a subquery", () => {
    expect(queryByExample("select * from (select * from t order by a) x", "a > 1")).toBe(
      "select * from (select * from (select * from t order by a) x) where a > 1",
    );
  });
});
