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
  | { type: "output"; data: string[] }
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

export type ObjectInfo = {
  name: string;
  objectType: string;
  status: string;
};

export type ColumnInfo = {
  name: string;
  dataType: string;
  nullable: boolean;
  dataDefault: string | null;
};

export type StatEntry = [string, string];

export type CommandSpec = {
  id: string;
  name: string;
  keys: string[];
  help: string;
  implemented: boolean;
};

export type WorkspaceFile = {
  name: string;
  sql: string;
};

export type Workspace = {
  version: number;
  files: WorkspaceFile[];
  active: number;
};

export type KeywordCase = "Upper" | "Lower" | "Keep";

export type FileText = {
  text: string;
  eol: string;
};

export type UpdateInfo = {
  version: string;
  currentVersion: string;
  notes: string | null;
  date: string | null;
};

export type UpdateProgress = {
  downloaded: number;
  total: number | null;
};

export type Template = {
  name: string;
  body: string;
};

export type FormatOptions = {
  indent: number;
  useTabs: boolean;
  keywordCase: KeywordCase;
  functionCase: KeywordCase;
  identifierCase: KeywordCase;
  style: "Standard" | "River";
  commaLeading: boolean;
  andOrIndent: number | null;
  contIndent: number | null;
  joinIndent: number | null;
  onNewLine: boolean;
  spaceAfterComma: boolean;
  spaceInParens: boolean;
  alignDeclarations: boolean;
  alignAssignments: boolean;
  blankAfterDeclare: boolean;
  blankBeforeException: boolean;
};

export type ConstraintInfo = {
  name: string;
  kind: string;
  status: string;
  columns: string[];
};

export type IndexInfo = {
  name: string;
  uniqueness: string;
  status: string;
  columns: string[];
};

export type TriggerInfo = {
  name: string;
  triggerType: string;
  event: string;
  status: string;
};

export type GrantInfo = {
  grantee: string;
  privilege: string;
  grantable: boolean;
};
