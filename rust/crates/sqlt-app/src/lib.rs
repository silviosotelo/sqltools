// SPDX-License-Identifier: GPL-3.0-only
//! Use cases shared by the Tauri shell and future frontends.

pub mod catalog;
pub mod state;
pub mod store;

pub use catalog::{list_columns, list_objects, list_table_names, load_ddl, session_stats};
pub use state::{AppState, HistoryEntry};
pub use store::{ConnectionConfig, ConnectionStore, StoreError};
