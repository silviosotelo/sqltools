/**
 * DDL builders for the "new object" form. They only produce text: the form never executes anything, the
 * statement lands in the editor so it can be reviewed (and it is the editor that runs it).
 */

export interface ColumnForm {
  name: string;
  /** Oracle type with its size, e.g. `VARCHAR2(40)`, `NUMBER(10,2)`, `DATE`. */
  type: string;
  nullable: boolean;
  defaultValue: string;
  primaryKey: boolean;
}

export interface TableForm {
  name: string;
  columns: ColumnForm[];
}

export interface SequenceForm {
  name: string;
  startWith: number;
  incrementBy: number;
  maxValue: string;
  cycle: boolean;
  cache: number;
}

const IDENT = /^[A-Za-z][A-Za-z0-9_$#]*$/;
const TYPE = /^[A-Za-z][A-Za-z0-9_ ]*(\([0-9]+(,[0-9]+)?\))?$/;

/** Quotes only when the name needs it; refuses anything that could break out of the statement. */
export function ident(name: string): string {
  const trimmed = name.trim();
  if (!trimmed || /["\u0000]/.test(trimmed)) throw new Error(`Unsafe identifier: ${name}`);
  return IDENT.test(trimmed) ? trimmed : `"${trimmed}"`;
}

export function tableDdl(form: TableForm): string {
  const name = ident(form.name);
  if (form.columns.length === 0) throw new Error("A table needs at least one column");
  const seen = new Set<string>();
  const lines = form.columns.map((column) => {
    const columnName = ident(column.name);
    if (seen.has(columnName.toUpperCase())) {
      throw new Error(`Duplicated column: ${column.name}`);
    }
    seen.add(columnName.toUpperCase());
    if (!TYPE.test(column.type.trim())) {
      throw new Error(`Invalid column type: ${column.type}`);
    }
    const parts = [columnName, column.type.trim().toUpperCase()];
    if (column.defaultValue.trim()) parts.push(`DEFAULT ${column.defaultValue.trim()}`);
    if (!column.nullable) parts.push("NOT NULL");
    return `  ${parts.join(" ")}`;
  });
  const primary = form.columns.filter((column) => column.primaryKey).map((column) => ident(column.name));
  if (primary.length > 0) {
    lines.push(`  CONSTRAINT ${ident(`${form.name.trim()}_PK`)} PRIMARY KEY (${primary.join(", ")})`);
  }
  return `create table ${name} (\n${lines.join(",\n")}\n);\n`;
}

export function sequenceDdl(form: SequenceForm): string {
  const name = ident(form.name);
  const parts = [`create sequence ${name}`];
  if (Number.isFinite(form.startWith)) parts.push(`start with ${Math.trunc(form.startWith)}`);
  if (Number.isFinite(form.incrementBy)) parts.push(`increment by ${Math.trunc(form.incrementBy)}`);
  const max = form.maxValue.trim().toUpperCase() || "NOMAXVALUE";
  if (max !== "NOMAXVALUE" && !/^[0-9]+$/.test(max)) throw new Error(`Invalid max value: ${form.maxValue}`);
  parts.push(max === "NOMAXVALUE" ? "nomaxvalue" : `maxvalue ${max}`);
  parts.push(form.cycle ? "cycle" : "nocycle");
  const cache = Math.trunc(form.cache);
  parts.push(cache > 0 ? `cache ${cache}` : "nocache");
  return `${parts.join(" ")};\n`;
}
