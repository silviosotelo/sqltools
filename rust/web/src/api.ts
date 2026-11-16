import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  ConnectionConfig,
  HistoryEntry,
  QueryEvent,
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
  listObjects: (connectionId: string) =>
    invoke<string[]>("list_objects", { connectionId }),
  explainPlan: (connectionId: string, sql: string) =>
    invoke<string>("explain_plan", { connectionId, sql }),
};
