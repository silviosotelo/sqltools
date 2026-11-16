import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  ColumnInfo,
  ConnectionConfig,
  ConstraintInfo,
  GrantInfo,
  HistoryEntry,
  IndexInfo,
  ObjectInfo,
  QueryEvent,
  StatEntry,
  TriggerInfo,
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
    onEvent: (event: QueryEvent) => void,
  ) => {
    const channel = new Channel<QueryEvent>();
    channel.onmessage = onEvent;
    return invoke<void>("execute", { connectionId, sql, maxRows, channel });
  },
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
  sessionStats: (connectionId: string) =>
    invoke<StatEntry[]>("session_stats", { connectionId }),
  formatSql: (sql: string, uppercase: boolean) =>
    invoke<string>("format_sql", { sql, uppercase }),
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
