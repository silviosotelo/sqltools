import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  ColumnInfo,
  CommandSpec,
  ConnectionConfig,
  ConstraintInfo,
  FileText,
  FormatOptions,
  GrantInfo,
  HistoryEntry,
  IndexInfo,
  ObjectInfo,
  QueryEvent,
  StatEntry,
  StatementRange,
  Template,
  TriggerInfo,
  UpdateInfo,
  UpdateProgress,
  Workspace,
} from "./types";

export const api = {
  listConnections: () => invoke<ConnectionConfig[]>("list_connections"),
  saveConnection: (config: ConnectionConfig, password: string) =>
    invoke<void>("save_connection", { config, password }),
  deleteConnection: (id: string) =>
    invoke<void>("delete_connection", { id }),
  connect: (id: string) => invoke<string>("connect", { id }),
  disconnect: (id: string) => invoke<void>("disconnect", { id }),
  execute: (
    connectionId: string,
    sql: string,
    maxRows: number,
    offset: number | null,
    haltOnError: boolean,
    onEvent: (event: QueryEvent) => void,
  ) => {
    const channel = new Channel<QueryEvent>();
    channel.onmessage = onEvent;
    return invoke<void>("execute", {
      connectionId,
      sql,
      maxRows,
      offset,
      haltOnError,
      channel,
    });
  },
  exitApp: () => invoke<void>("exit_app"),
  revealFileLocation: (path: string) =>
    invoke<void>("reveal_file_location", { path }),
  openSqlHelp: () => invoke<void>("open_sql_help"),
  toggleReadOnly: (id: string) => invoke<boolean>("toggle_read_only", { id }),
  enableDbmsOutput: (connectionId: string) =>
    invoke<void>("enable_dbms_output", { connectionId }),
  cancel: (id: string) => invoke<void>("cancel", { id }),
  commit: (id: string) => invoke<void>("commit", { id }),
  rollback: (id: string) => invoke<void>("rollback", { id }),
  history: () => invoke<HistoryEntry[]>("history"),
  listTableNames: (connectionId: string) =>
    invoke<string[]>("list_table_names", { connectionId }),
  listObjects: (
    connectionId: string,
    schema: string | null,
    objectType: string | null,
  ) =>
    invoke<ObjectInfo[]>("list_objects", {
      connectionId,
      schema,
      objectType,
    }),
  listColumns: (
    connectionId: string,
    schema: string | null,
    table: string,
  ) =>
    invoke<ColumnInfo[]>("list_columns", {
      connectionId,
      schema,
      table,
    }),
  loadDdl: (
    connectionId: string,
    schema: string | null,
    name: string,
    objectType: string,
  ) =>
    invoke<string>("load_ddl", { connectionId, schema, name, objectType }),
  loadDdlMeta: (connectionId: string, schema: string | null, name: string) =>
    invoke<string>("load_ddl_meta", { connectionId, schema, name }),
  sessionStats: (connectionId: string) =>
    invoke<StatEntry[]>("session_stats", { connectionId }),
  listCommands: () => invoke<CommandSpec[]>("list_commands"),
  pickSqlFile: () => invoke<string | null>("pick_sql_file"),
  pickSqlSavePath: (suggested: string | null) =>
    invoke<string | null>("pick_sql_save_path", { suggested }),
  statementRanges: (sql: string) =>
    invoke<StatementRange[]>("statement_ranges", { sql }),
  listTemplates: () => invoke<string[]>("list_templates"),
  loadTemplate: (name: string) =>
    invoke<Template | null>("load_template", { name }),
  saveTemplate: (name: string, body: string) =>
    invoke<void>("save_template", { name, body }),
  deleteTemplate: (name: string) =>
    invoke<void>("delete_template", { name }),
  readSqlFile: (path: string) => invoke<FileText>("read_sql_file", { path }),
  writeSqlFile: (path: string, text: string, eol: string) =>
    invoke<void>("write_sql_file", { path, text, eol }),
  saveWorkspace: (workspace: Workspace) =>
    invoke<void>("save_workspace", { workspace }),
  saveWorkspaceTo: (path: string, workspace: Workspace) =>
    invoke<void>("save_workspace_to", { path, workspace }),
  pickWorkspaceSavePath: (suggested: string | null) =>
    invoke<string | null>("pick_workspace_save_path", { suggested }),
  loadWorkspace: () => invoke<Workspace>("load_workspace"),
  migrateLegacy: (path: string) =>
    invoke<number>("migrate_legacy_connections", { path }),
  exportCsvFile: (
    connectionId: string,
    sql: string,
    maxRows: number,
  ) => invoke<string>("export_csv", { connectionId, sql, maxRows }),
  checkUpdate: () => invoke<UpdateInfo | null>("check_update"),
  installUpdate: (onProgress: (progress: UpdateProgress) => void) => {
    const channel = new Channel<UpdateProgress>();
    channel.onmessage = onProgress;
    return invoke<void>("install_update", { channel });
  },
  formatSql: (sql: string, uppercase: boolean) =>
    invoke<string>("format_sql", { sql, uppercase }),
  formatSqlWith: (sql: string, options: FormatOptions) =>
    invoke<string>("format_sql_with", { sql, options }),
  inferFormatProfile: (sql: string) =>
    invoke<FormatOptions>("infer_format_profile", { sql }),
  listFormatProfiles: () => invoke<string[]>("list_format_profiles"),
  saveFormatProfile: (name: string, options: FormatOptions) =>
    invoke<void>("save_format_profile", { name, options }),
  loadFormatProfile: (name: string) =>
    invoke<FormatOptions>("load_format_profile", { name }),
  deleteFormatProfile: (name: string) =>
    invoke<void>("delete_format_profile", { name }),
  listConstraints: (
    connectionId: string,
    schema: string | null,
    table: string,
  ) =>
    invoke<ConstraintInfo[]>("list_constraints", {
      connectionId,
      schema,
      table,
    }),
  listIndexes: (connectionId: string, schema: string | null, table: string) =>
    invoke<IndexInfo[]>("list_indexes", {
      connectionId,
      schema,
      table,
    }),
  listTriggers: (connectionId: string, schema: string | null, table: string) =>
    invoke<TriggerInfo[]>("list_triggers", {
      connectionId,
      schema,
      table,
    }),
  listGrants: (connectionId: string, schema: string | null, table: string) =>
    invoke<GrantInfo[]>("list_grants", {
      connectionId,
      schema,
      table,
    }),
  explainPlan: (connectionId: string, sql: string) =>
    invoke<string>("explain_plan", { connectionId, sql }),
};
