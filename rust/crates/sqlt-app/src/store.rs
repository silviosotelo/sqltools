// SPDX-License-Identifier: GPL-3.0-only
//! Connection store: JSON metadata plus passwords in the OS keyring.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sqlt_oracle::{ConnectMode, ConnectSpec, ConnectTarget};

const KEYRING_SERVICE: &str = "com.sqltools.rust";

#[derive(Debug)]
pub enum StoreError {
    Io(String),
    Json(String),
    Keyring(String),
    NotFound(String),
    Invalid(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io(msg)
            | StoreError::Json(msg)
            | StoreError::Keyring(msg)
            | StoreError::NotFound(msg)
            | StoreError::Invalid(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<std::io::Error> for StoreError {
    fn from(error: std::io::Error) -> Self {
        StoreError::Io(error.to_string())
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(error: serde_json::Error) -> Self {
        StoreError::Json(error.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionConfig {
    pub id: String,
    pub label: String,
    pub user: String,
    pub alias: Option<String>,
    pub connect: Option<String>,
    pub tns_admin: Option<String>,
    pub oci_dir: Option<String>,
    pub mode: String,
    pub read_only: bool,
    pub prefetch: u32,
    pub save_password: bool,
}

impl ConnectionConfig {
    pub fn new(label: &str, user: &str) -> Self {
        ConnectionConfig {
            id: format!(
                "{}-{}",
                label.to_ascii_lowercase().replace([' ', '/', '\\'], "-"),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or_default()
            ),
            label: label.to_string(),
            user: user.to_string(),
            alias: None,
            connect: None,
            tns_admin: None,
            oci_dir: None,
            mode: "NORMAL".to_string(),
            read_only: false,
            prefetch: 1000,
            save_password: true,
        }
    }
}

pub struct ConnectionStore {
    path: PathBuf,
    connections: Vec<ConnectionConfig>,
}

impl ConnectionStore {
    pub fn new(path: PathBuf) -> Self {
        ConnectionStore {
            path,
            connections: Vec::new(),
        }
    }

    pub fn load() -> Result<Self, StoreError> {
        Self::load_from(default_config_path())
    }

    pub fn load_from(path: PathBuf) -> Result<Self, StoreError> {
        let connections = if path.is_file() {
            let text = fs::read_to_string(&path)?;
            if text.trim().is_empty() {
                Vec::new()
            } else {
                serde_json::from_str(&text)?
            }
        } else {
            Vec::new()
        };
        Ok(ConnectionStore { path, connections })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn list(&self) -> Vec<ConnectionConfig> {
        self.connections.clone()
    }

    pub fn get(&self, id: &str) -> Option<&ConnectionConfig> {
        self.connections.iter().find(|c| c.id == id)
    }

    pub fn upsert(&mut self, config: ConnectionConfig) -> Result<(), StoreError> {
        if config.user.trim().is_empty() {
            return Err(StoreError::Invalid("User is required".to_string()));
        }
        if config.label.trim().is_empty() {
            return Err(StoreError::Invalid("Label is required".to_string()));
        }
        if config
            .alias
            .as_deref()
            .map(str::trim)
            .unwrap_or("")
            .is_empty()
            && config
                .connect
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
        {
            return Err(StoreError::Invalid(
                "Alias or direct connection is required".to_string(),
            ));
        }
        if let Some(existing) = self.connections.iter_mut().find(|c| c.id == config.id) {
            *existing = config;
        } else {
            self.connections.push(config);
        }
        self.save()
    }

    pub fn remove(&mut self, id: &str) -> Result<(), StoreError> {
        let save_password = self.get(id).map(|c| c.save_password).unwrap_or(false);
        let before = self.connections.len();
        self.connections.retain(|c| c.id != id);
        if self.connections.len() == before {
            return Err(StoreError::NotFound(format!("Connection {id} not found")));
        }
        if save_password {
            let _ = self.delete_password(id);
        }
        self.save()
    }

    fn save(&self) -> Result<(), StoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(&self.connections)?;
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, text)?;
        fs::rename(tmp, &self.path)?;
        Ok(())
    }

    pub fn set_password(&self, id: &str, password: &str) -> Result<(), StoreError> {
        keyring_entry(id)?
            .set_password(password)
            .map_err(|e| StoreError::Keyring(e.to_string()))
    }

    pub fn password(&self, id: &str) -> Result<String, StoreError> {
        keyring_entry(id)?
            .get_password()
            .map_err(|e| StoreError::Keyring(e.to_string()))
    }

    pub fn delete_password(&self, id: &str) -> Result<(), StoreError> {
        keyring_entry(id)?
            .delete_credential()
            .map_err(|e| StoreError::Keyring(e.to_string()))
    }

    pub fn to_spec(&self, id: &str) -> Result<ConnectSpec, StoreError> {
        let config = self
            .get(id)
            .ok_or_else(|| StoreError::NotFound(format!("Connection {id} not found")))?;
        let password = self.password(id)?;
        let target = match config
            .connect
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(connect) => ConnectTarget::Direct(connect.to_string()),
            None => {
                let alias = config
                    .alias
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| StoreError::Invalid("Alias is required".to_string()))?;
                ConnectTarget::Alias(alias.to_string())
            }
        };
        let mode = ConnectMode::parse(&config.mode)
            .ok_or_else(|| StoreError::Invalid(format!("Invalid mode: {}", config.mode)))?;
        Ok(ConnectSpec {
            user: config.user.clone(),
            password,
            target,
            mode,
            read_only: config.read_only,
            oci_dir: config.oci_dir.as_ref().map(PathBuf::from),
            tns_admin: config.tns_admin.as_ref().map(PathBuf::from),
            prefetch: config.prefetch.max(1),
        })
    }
}

fn keyring_entry(id: &str) -> Result<keyring::Entry, StoreError> {
    keyring::Entry::new(KEYRING_SERVICE, id).map_err(|e| StoreError::Keyring(e.to_string()))
}

fn default_config_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        base.join("SQLToolsRust").join("connections.json")
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        home.join("Library/Application Support/SQLToolsRust")
            .join("connections.json")
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".config")
            });
        base.join("SQLToolsRust").join("connections.json")
    }
}

pub fn config_dir() -> PathBuf {
    default_config_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(name: &str) -> (PathBuf, ConnectionStore) {
        let path =
            std::env::temp_dir().join(format!("sqlt-store-{}-{name}.json", std::process::id()));
        let _ = fs::remove_file(&path);
        (path.clone(), ConnectionStore::new(path))
    }

    #[test]
    fn json_roundtrip_without_password() {
        let (path, mut store) = temp_store("roundtrip");
        let mut config = ConnectionConfig::new("Local", "scott");
        config.alias = Some("local".to_string());
        store.upsert(config.clone()).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(!text.contains("\"password\""));
        let loaded = ConnectionStore::load_from(path.clone()).unwrap();
        assert_eq!(loaded.list(), vec![config]);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn update_and_remove() {
        let (path, mut store) = temp_store("update");
        let mut config = ConnectionConfig::new("One", "u1");
        config.alias = Some("a".to_string());
        store.upsert(config.clone()).unwrap();
        let mut updated = config.clone();
        updated.label = "Two".to_string();
        store.upsert(updated.clone()).unwrap();
        assert_eq!(store.list(), vec![updated.clone()]);
        store.remove(&updated.id).unwrap();
        assert!(store.list().is_empty());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn validates_required_fields() {
        let (_path, mut store) = temp_store("validation");
        let config = ConnectionConfig::new("", "u");
        assert!(store.upsert(config).is_err());
        let config = ConnectionConfig::new("Label", "");
        assert!(store.upsert(config).is_err());
        let config = ConnectionConfig::new("Label", "u");
        assert!(store.upsert(config).is_err());
    }
}
