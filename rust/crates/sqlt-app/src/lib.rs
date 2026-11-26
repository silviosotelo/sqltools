// SPDX-License-Identifier: GPL-3.0-only
//! Use cases shared by the Tauri shell and future frontends.

pub mod browser;
pub mod catalog;
pub mod commands;
pub mod export;
pub mod fileio;
pub mod migrate;
pub mod profiles;
pub mod script;
pub mod sqlplus;
pub mod state;
pub mod store;
pub mod templates;
pub mod workspace;

pub use browser::{
    compile_invalid_objects, compile_object, ddl_ident, list_dependencies, list_errors,
    list_invalid_objects, set_constraint_enabled, set_trigger_enabled, CompileError, Dependency,
};
pub use catalog::{
    count_objects, list_columns, list_constraints, list_grants, list_indexes, list_objects,
    list_table_names, list_triggers, load_ddl, load_table_ddl_meta, session_stats,
    transaction_pending,
};
pub use commands::{registry, CommandSpec};
pub use state::{AppState, HistoryEntry};
pub use store::{
    config_dir, ConnectionConfig, ConnectionStore, Environment, StoreError, ENV_COLORS,
};
pub use workspace::{Workspace, WorkspaceFile};
