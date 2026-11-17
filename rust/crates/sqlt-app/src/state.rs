// SPDX-License-Identifier: GPL-3.0-only
//! Application state: sessions, history and the connection store.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sqlt_oracle::{ConnectSpec, QueryOptions, Session};

use crate::store::{ConnectionStore, StoreError};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub connection_id: String,
    pub sql: String,
    pub rows: usize,
    pub elapsed_ms: u64,
    pub error: Option<String>,
    pub started_at: u64,
}

pub struct AppState {
    pub store: Mutex<ConnectionStore>,
    pub sessions: Mutex<HashMap<String, Arc<Session>>>,
    pub history: Mutex<Vec<HistoryEntry>>,
}

impl AppState {
    pub fn new() -> Result<Self, StoreError> {
        Ok(AppState::with_store(ConnectionStore::load()?))
    }

    pub fn with_store(store: ConnectionStore) -> Self {
        AppState {
            store: Mutex::new(store),
            sessions: Mutex::new(HashMap::new()),
            history: Mutex::new(Vec::new()),
        }
    }

    pub fn connect(&self, connection_id: &str) -> Result<String, String> {
        let spec = {
            let store = self
                .store
                .lock()
                .map_err(|_| "Connection store lock poisoned".to_string())?;
            store.to_spec(connection_id).map_err(|e| e.to_string())?
        };
        // Open first so a failed reconnect keeps the existing session usable.
        let session = Session::open(&spec).map_err(|e| e.to_string())?;
        let version = session.server_version().map_err(|e| e.to_string())?;
        self.close_existing(connection_id)?;
        self.sessions
            .lock()
            .map_err(|_| "Session map lock poisoned".to_string())?
            .insert(connection_id.to_string(), Arc::new(session));
        Ok(version)
    }

    pub fn disconnect(&self, connection_id: &str) -> Result<(), String> {
        self.close_existing(connection_id)
    }

    /// Resolved connection (user, password, target, mode) without opening a
    /// session; "Execute in SQL*Plus" needs it for the external command line.
    pub fn connect_spec(&self, connection_id: &str) -> Result<ConnectSpec, String> {
        let store = self
            .store
            .lock()
            .map_err(|_| "Connection store lock poisoned".to_string())?;
        store.to_spec(connection_id).map_err(|e| e.to_string())
    }

    /// Cancels a running script, waits for its cycle to finish and only then
    /// drops the session, so no statement keeps running on an orphaned
    /// connection.
    fn close_existing(&self, connection_id: &str) -> Result<(), String> {
        let existing = self
            .sessions
            .lock()
            .map_err(|_| "Session map lock poisoned".to_string())?
            .get(connection_id)
            .cloned();
        if let Some(session) = existing {
            session.close();
            // `close` interrupts the statement; the guard waits for the script
            // to unwind so nothing writes to the old session afterwards.
            let _guard = session.lock_execution();
            self.sessions
                .lock()
                .map_err(|_| "Session map lock poisoned".to_string())?
                .remove(connection_id);
        }
        Ok(())
    }

    pub fn session(&self, connection_id: &str) -> Result<Arc<Session>, String> {
        self.sessions
            .lock()
            .map_err(|_| "Session map lock poisoned".to_string())?
            .get(connection_id)
            .cloned()
            .ok_or_else(|| format!("Connection {connection_id} is not connected"))
    }

    pub fn query_options(&self, connection_id: &str) -> QueryOptions {
        let prefetch = self
            .store
            .lock()
            .ok()
            .and_then(|store| store.get(connection_id).map(|c| c.prefetch))
            .unwrap_or(1000);
        QueryOptions {
            prefetch: prefetch.max(1),
            ..QueryOptions::default()
        }
    }

    pub fn record(&self, entry: HistoryEntry) {
        if let Ok(mut history) = self.history.lock() {
            history.push(entry);
            if history.len() > 2000 {
                let excess = history.len() - 2000;
                history.drain(0..excess);
            }
        }
    }

    pub fn history(&self) -> Vec<HistoryEntry> {
        self.history
            .lock()
            .map(|entries| entries.clone())
            .unwrap_or_default()
    }

    /// Flips the stored read-only flag. The caller reconnects for it to take
    /// effect, because the driver enforces it when the session opens.
    pub fn toggle_read_only(&self, connection_id: &str) -> Result<bool, String> {
        let mut store = self
            .store
            .lock()
            .map_err(|_| "Connection store lock poisoned".to_string())?;
        let mut config = store
            .get(connection_id)
            .cloned()
            .ok_or_else(|| format!("Connection {connection_id} not found"))?;
        config.read_only = !config.read_only;
        let value = config.read_only;
        store.upsert(config).map_err(|e| e.to_string())?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::ConnectionConfig;

    #[test]
    fn toggles_read_only_and_persists_it() {
        let path = std::env::temp_dir().join(format!("sqlt-state-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut store = ConnectionStore::new(path.clone());
        let mut config = ConnectionConfig::new("Local", "scott");
        config.alias = Some("x".to_string());
        config.save_password = false;
        store.upsert(config.clone()).unwrap();
        let state = AppState::with_store(store);
        assert!(state.toggle_read_only(&config.id).unwrap());
        assert!(
            state
                .store
                .lock()
                .unwrap()
                .get(&config.id)
                .unwrap()
                .read_only
        );
        assert!(!state.toggle_read_only(&config.id).unwrap());
        assert!(state.toggle_read_only("missing").is_err());
        let _ = std::fs::remove_file(path);
    }
}
