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
  AiConfig,
  CompileError,
  CompareResult,
  DebugPosition,
  FileEntry,
  Project,
  Report,
  SchedulerJob,
  Dependency,
  ObjectCount,
  ObjectInfo,
  ProfileResult,
  TodoItem,
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
  executeInSqlPlus: (id: string, script: string) =>
    invoke<string>("execute_in_sqlplus", { id, script }),
  tableTransformation: (id: string, schema: string | null, table: string) =>
    invoke<string>("table_transformation_script", { id, schema, table }),
  execute: (
    connectionId: string,
    sql: string,
    maxRows: number,
    offset: number | null,
    haltOnError: boolean,
    onEvent: (event: QueryEvent) => void,
    historySql?: string,
  ) => {
    const channel = new Channel<QueryEvent>();
    channel.onmessage = onEvent;
    return invoke<void>("execute", {
      connectionId,
      sql,
      historySql: historySql ?? null,
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
  updateCell: (
    connectionId: string,
    table: string,
    column: string,
    rowid: string,
    oldValue: string | null,
    newValue: string | null,
  ) =>
    invoke<boolean>("update_cell", {
      connectionId,
      table,
      column,
      rowid,
      oldValue,
      newValue,
    }),
  transactionPending: (connectionId: string) =>
    invoke<boolean>("transaction_pending", { connectionId }),
  history: () => invoke<HistoryEntry[]>("history"),
  listDependencies: (
    connectionId: string,
    schema: string | null,
    name: string,
    objectType: string,
    dependents: boolean,
  ) =>
    invoke<Dependency[]>("list_dependencies", {
      connectionId,
      schema,
      name,
      objectType,
      dependents,
    }),
  listInvalidObjects: (connectionId: string, schema: string | null) =>
    invoke<ObjectInfo[]>("list_invalid_objects", { connectionId, schema }),
  compileObject: (
    connectionId: string,
    schema: string | null,
    name: string,
    objectType: string,
  ) =>
    invoke<void>("compile_object", {
      connectionId,
      schema,
      name,
      objectType,
    }),
  compileInvalidObjects: (connectionId: string, schema: string | null) =>
    invoke<number[]>("compile_invalid_objects", { connectionId, schema }),
  listCompileErrors: (
    connectionId: string,
    schema: string | null,
    name: string,
    objectType: string,
  ) =>
    invoke<CompileError[]>("list_compile_errors", {
      connectionId,
      schema,
      name,
      objectType,
    }),
  toggleTrigger: (
    connectionId: string,
    schema: string | null,
    name: string,
    enabled: boolean,
  ) =>
    invoke<void>("toggle_trigger", { connectionId, schema, name, enabled }),
  toggleConstraint: (
    connectionId: string,
    schema: string | null,
    table: string,
    name: string,
    enabled: boolean,
  ) =>
    invoke<void>("toggle_constraint", {
      connectionId,
      schema,
      table,
      name,
      enabled,
    }),
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
  aiConfig: () => invoke<AiConfig>("ai_config"),
  aiSetConfig: (config: AiConfig) => invoke<void>("ai_set_config", { config }),
  aiHasKey: () => invoke<boolean>("ai_has_key"),
  aiSetKey: (key: string | null) => invoke<void>("ai_set_key", { key }),
  aiRun: (fn: string, code: string) => invoke<string>("ai_run", { function: fn, code }),
  listSchedulerJobs: (connectionId: string, schema: string | null) =>
    invoke<SchedulerJob[]>("list_scheduler_jobs", { connectionId, schema }),
  listProjects: () => invoke<Project[]>("list_projects"),
  saveProject: (project: Project) => invoke<void>("save_project", { project }),
  loadProject: (name: string) => invoke<Project>("load_project", { name }),
  deleteProject: (name: string) => invoke<void>("delete_project", { name }),
  listReports: () => invoke<Report[]>("list_reports"),
  saveReport: (name: string, sql: string) => invoke<void>("save_report", { name, sql }),
  deleteReport: (name: string) => invoke<void>("delete_report", { name }),
  listFileLocations: () => invoke<string[]>("list_file_locations"),
  addFileLocation: (path: string) => invoke<string[]>("add_file_location", { path }),
  listDirectory: (path: string) => invoke<FileEntry[]>("list_directory", { path }),
  pickFolder: () => invoke<string | null>("pick_folder"),
  debugStart: (connectionId: string, sql: string) =>
    invoke<DebugPosition>("debug_start", { connectionId, sql }),
  debugStep: (connectionId: string, mode: "over" | "into" | "out") =>
    invoke<DebugPosition>("debug_step", { connectionId, mode }),
  debugWatch: (connectionId: string, name: string) =>
    invoke<string | null>("debug_watch", { connectionId, name }),
  debugBreakpoint: (connectionId: string, owner: string, name: string, namespace: number, line: number) =>
    invoke<number>("debug_breakpoint", { connectionId, owner, name, namespace, line }),
  debugStop: (connectionId: string) => invoke<void>("debug_stop", { connectionId }),
  profileSql: (connectionId: string, sql: string) =>
    invoke<ProfileResult>("profile_sql", { connectionId, sql }),
  listTodos: (connectionId: string, schema: string | null) =>
    invoke<TodoItem[]>("list_todos", { connectionId, schema }),
  saveSourcesSnapshot: (connectionId: string, schema: string | null, path: string) =>
    invoke<number>("save_sources_snapshot", { connectionId, schema, path }),
  compareWithSnapshot: (connectionId: string, schema: string | null, path: string) =>
    invoke<CompareResult>("compare_with_snapshot", { connectionId, schema, path }),
  suggestObjects: (connectionId: string, schema: string | null) =>
    invoke<ObjectInfo[]>("suggest_objects", { connectionId, schema }),
  countObjects: (connectionId: string, schema: string | null) =>
    invoke<ObjectCount[]>("count_objects", { connectionId, schema }),
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
  extractSchemaDdl: (
    connectionId: string,
    schema: string | null,
    objectType: string | null,
    limit: number,
    path: string,
  ) =>
    invoke<number>("extract_schema_ddl", {
      connectionId,
      schema,
      objectType,
      limit,
      path,
    }),
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
