// SPDX-License-Identifier: GPL-3.0-only
//! Use cases shared by the Tauri shell and future frontends.

pub mod state;
pub mod store;

pub use state::{AppState, HistoryEntry};
pub use store::{ConnectionConfig, ConnectionStore, StoreError};
