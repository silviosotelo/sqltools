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
const TYPE = /^([A-Za-z][A-Za-z0-9_ ]*?)(\([0-9]+(,[0-9]+)?\))?$/;
/** Types this form can build with confidence; anything else would be a guess. */
const BASE_TYPES = new Set([
  "CHAR",
  "VARCHAR",
  "VARCHAR2",
  "NCHAR",
  "NVARCHAR2",
  "NUMBER",
  "INTEGER",
  "INT",
  "SMALLINT",
  "FLOAT",
  "BINARY_FLOAT",
  "BINARY_DOUBLE",
  "DATE",
  "CLOB",
  "NCLOB",
  "BLOB",
  "RAW",
  "LONG",
  "ROWID",
]);
const SIZED = new Set(["CHAR", "VARCHAR", "VARCHAR2", "NCHAR", "NVARCHAR2", "NUMBER", "FLOAT", "RAW"]);

/** A type name Oracle knows, with its optional size. `FOO(1)` would only fail once the script ran. */
function checkedType(raw: string): string {
  const type = raw.trim().replace(/\s+/g, " ");
  const match = TYPE.exec(type);
  if (!match) throw new Error(`Invalid column type: ${raw}`);
  const base = match[1].trim().toUpperCase();
  const known =
    BASE_TYPES.has(base) ||
    /^(TIMESTAMP( WITH (LOCAL )?TIME ZONE)?|INTERVAL (YEAR|DAY) TO (MONTH|SECOND))$/.test(base);
  if (!known) throw new Error(`Unsupported column type: ${raw}`);
  if (match[2] && !SIZED.has(base)) throw new Error(`A size is not valid for ${base}`);
  return type.toUpperCase();
}

/** One expression: a top-level comma would add a column and a semicolon would end the statement. */
function unsafeExpression(text: string): boolean {
  let depth = 0;
  let quote: string | null = null;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (quote) {
      if (ch === quote) quote = null;
      continue;
    }
    if (ch === "'" || ch === '"') {
      quote = ch;
    } else if (ch === "(") {
      depth++;
    } else if (ch === ")") {
      depth--;
    } else if (depth === 0 && (ch === ";" || ch === ",")) {
      return true;
    } else if ((ch === "-" && text[i + 1] === "-") || (ch === "/" && text[i + 1] === "*")) {
      return true;
    }
  }
  return false;
}

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
    const parts = [columnName, checkedType(column.type)];
    if (column.defaultValue.trim()) {
      if (unsafeExpression(column.defaultValue)) {
        throw new Error(`Invalid default value: ${column.defaultValue}`);
      }
      parts.push(`DEFAULT ${column.defaultValue.trim()}`);
    }
    if (!column.nullable) parts.push("NOT NULL");
    return `  ${parts.join(" ")}`;
  });
  const primary = form.columns.filter((column) => column.primaryKey).map((column) => ident(column.name));
  if (primary.length > 0) {
    lines.push(`  CONSTRAINT ${ident(`${form.name.trim()}_PK`)} PRIMARY KEY (${primary.join(", ")})`);
  }
  return `create table ${name} (\n${lines.join(",\n")}\n);\n`;
}

function whole(value: number, label: string): number {
  if (!Number.isInteger(value)) throw new Error(`${label} must be a whole number`);
  return value;
}

export function sequenceDdl(form: SequenceForm): string {
  const name = ident(form.name);
  const start = whole(form.startWith, "Start with");
  const increment = whole(form.incrementBy, "Increment by");
  if (increment === 0) throw new Error("Increment by cannot be zero");
  const max = form.maxValue.trim().toUpperCase() || "NOMAXVALUE";
  if (max !== "NOMAXVALUE" && !/^-?[0-9]+$/.test(max)) {
    throw new Error(`Invalid max value: ${form.maxValue}`);
  }
  // NOMAXVALUE/NOMINVALUE fix the opposite limit; outside it Oracle rejects the statement (ORA-04009).
  if (max === "NOMAXVALUE") {
    if (increment > 0 && start < 1) throw new Error("An ascending sequence starts at 1 or more");
    if (increment < 0 && start > -1) throw new Error("A descending sequence starts at -1 or less");
  }
  const cache = whole(form.cache, "Cache");
  if (cache < 0) throw new Error("Cache cannot be negative");
  const parts = [
    `create sequence ${name}`,
    `start with ${start}`,
    `increment by ${increment}`,
    max === "NOMAXVALUE" ? "nomaxvalue" : `maxvalue ${max}`,
    form.cycle ? "cycle" : "nocycle",
    cache > 0 ? `cache ${cache}` : "nocache",
  ];
  return `${parts.join(" ")};\n`;
}
