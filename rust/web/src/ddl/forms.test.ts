import { describe, expect, it } from "vitest";
import { ident, sequenceDdl, tableDdl } from "./forms";

const column = (over: Partial<Parameters<typeof tableDdl>[0]["columns"][number]> = {}) => ({
  name: "ID",
  type: "NUMBER",
  nullable: false,
  defaultValue: "",
  primaryKey: false,
  ...over,
});

describe("ident", () => {
  it("leaves plain names alone and quotes the rest", () => {
    expect(ident("A11")).toBe("A11");
    expect(ident("mi tabla")).toBe('"mi tabla"');
    expect(() => ident('a"b')).toThrow();
    expect(() => ident("  ")).toThrow();
  });
});

describe("tableDdl", () => {
  it("builds a table with defaults, nullability and a primary key", () => {
    const ddl = tableDdl({
      name: "CLIENTES",
      columns: [
        column({ name: "ID", type: "NUMBER(10)", primaryKey: true }),
        column({ name: "NOMBRE", type: "VARCHAR2(60)", nullable: true, defaultValue: "'sin nombre'" }),
      ],
    });
    expect(ddl).toBe(
      "create table CLIENTES (\n" +
        "  ID NUMBER(10) NOT NULL,\n" +
        "  NOMBRE VARCHAR2(60) DEFAULT 'sin nombre',\n" +
        "  CONSTRAINT CLIENTES_PK PRIMARY KEY (ID)\n" +
        ");\n",
    );
  });
  it("refuses an empty table, duplicated columns and invalid types", () => {
    expect(() => tableDdl({ name: "T", columns: [] })).toThrow();
    expect(() =>
      tableDdl({ name: "T", columns: [column(), column({ name: "id" })] }),
    ).toThrow(/Duplicated/);
    expect(() => tableDdl({ name: "T", columns: [column({ type: "NUMBER); drop table x --" })] })).toThrow();
  });
});

describe("sequenceDdl", () => {
  it("builds a sequence with its options", () => {
    expect(
      sequenceDdl({ name: "SEQ", startWith: 10, incrementBy: 2, maxValue: "", cycle: true, cache: 20 }),
    ).toBe("create sequence SEQ start with 10 increment by 2 nomaxvalue cycle cache 20;\n");
  });
  it("supports maxvalue and nocache", () => {
    expect(
      sequenceDdl({ name: "SEQ", startWith: 1, incrementBy: 1, maxValue: "999", cycle: false, cache: 0 }),
    ).toBe("create sequence SEQ start with 1 increment by 1 maxvalue 999 nocycle nocache;\n");
    expect(() =>
      sequenceDdl({ name: "SEQ", startWith: 1, incrementBy: 1, maxValue: "1; drop", cycle: false, cache: 0 }),
    ).toThrow(/Invalid max/);
  });
});
