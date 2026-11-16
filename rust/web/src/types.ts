export type Value =
  | { kind: "null" }
  | { kind: "text"; value: string }
  | { kind: "number"; value: string }
  | { kind: "date"; value: string }
  | { kind: "timestamp"; value: string }
  | { kind: "binary"; value: number[] };

export type ColumnMeta = {
  name: string;
  typeName: string;
  nullable: boolean;
  numeric: boolean;
};

export type Batch = {
  columns: ColumnMeta[];
  rows: Value[][];
};

export type QueryEvent =
  | { type: "columns"; data: ColumnMeta[] }
  | { type: "batch"; data: Batch }
  | { type: "done"; data: { rows: number; elapsedMs: number } }
  | { type: "error"; data: string };

export type ConnectionConfig = {
  id: string;
  label: string;
  user: string;
  alias: string | null;
  connect: string | null;
  tnsAdmin: string | null;
  ociDir: string | null;
  mode: "NORMAL" | "SYSDBA" | "SYSOPER";
  readOnly: boolean;
  prefetch: number;
  savePassword: boolean;
};

export type HistoryEntry = {
  connectionId: string;
  sql: string;
  rows: number;
  elapsedMs: number;
  error: string | null;
  startedAt: number;
};
