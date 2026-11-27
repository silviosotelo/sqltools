import { describe, expect, it } from "vitest";
import {
  limitSuggestions,
  statementAt,
  tableForQualifier,
  tablesInScope,
  tokenize,
} from "./completion";

describe("tokenize", () => {
  it("ignores string literals, quoted identifiers and comments", () => {
    const tokens = tokenize("select 'from t' as \"from\" from a -- from b\n/* from c */ where x = 1");
    const words = tokens.map((token) => token.lower);
    expect(words).toContain("from");
    expect(words.filter((word) => word === "from")).toHaveLength(1);
    expect(words).not.toContain("b");
    expect(words).not.toContain("c");
    expect(words).toContain("a");
  });
});

describe("tablesInScope", () => {
  it("reads the table of a plain from", () => {
    expect(tablesInScope("select * from a11 where x = 1")).toEqual([{ table: "a11" }]);
  });
  it("reads an alias, with or without AS, and does not mistake a keyword for one", () => {
    expect(tablesInScope("select * from a11 t where x = 1")).toEqual([{ table: "a11", alias: "t" }]);
    expect(tablesInScope("select * from a11 as t")).toEqual([{ table: "a11", alias: "t" }]);
    expect(tablesInScope("select * from a11 where x = 1")).toEqual([{ table: "a11" }]);
  });
  it("reads owner.table, joins and comma lists", () => {
    expect(tablesInScope("select * from anamnesis.a11")).toEqual([{ table: "anamnesis.a11" }]);
    expect(tablesInScope("select * from a x join b y on x.id = y.id")).toEqual([
      { table: "a", alias: "x" },
      { table: "b", alias: "y" },
    ]);
    expect(tablesInScope("select * from a, b c where a.id = c.id")).toEqual([
      { table: "a" },
      { table: "b", alias: "c" },
    ]);
  });
  it("skips a subquery as a table but still sees the tables inside it", () => {
    expect(tablesInScope("select * from (select * from t) x")).toEqual([{ table: "t" }]);
    expect(tablesInScope("update a11 set n = 1")).toEqual([{ table: "a11" }]);
  });
});

describe("tableForQualifier", () => {
  const sql = "select * from a11 t join activo_fijo af on t.id = af.id";
  it("resolves an alias before a table name", () => {
    expect(tableForQualifier(sql, "t")).toBe("a11");
    expect(tableForQualifier(sql, "AF")).toBe("activo_fijo");
  });
  it("resolves a table by its own name and returns null when unknown", () => {
    expect(tableForQualifier("select * from a11", "A11")).toBe("a11");
    expect(tableForQualifier(sql, "nope")).toBeNull();
  });
});

describe("statementAt", () => {
  it("returns only the statement around the cursor", () => {
    const sql = "select * from a;\nselect * from b where x = 1;select * from c";
    expect(statementAt(sql, sql.indexOf("where"))).toBe("\nselect * from b where x = 1");
  });
});

describe("limitSuggestions", () => {
  it("removes duplicates and caps the list", () => {
    const items = ["a", "a", ...Array.from({ length: 200 }, (_, i) => `x${i}`)];
    const result = limitSuggestions(items, 100);
    expect(result).toHaveLength(100);
    expect(result[0]).toBe("a");
    expect(new Set(result).size).toBe(result.length);
  });
});
