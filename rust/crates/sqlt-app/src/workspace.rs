// SPDX-License-Identifier: GPL-3.0-only
//! Minimal workspace persistence: SQL buffers and the active buffer index.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::store::{config_dir, StoreError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceFile {
    pub name: String,
    pub sql: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub version: u32,
    pub files: Vec<WorkspaceFile>,
    pub active: usize,
}

impl Workspace {
    pub fn new(name: &str, sql: &str) -> Self {
        Workspace {
            version: 1,
            files: vec![WorkspaceFile {
                name: name.to_string(),
                sql: sql.to_string(),
            }],
            active: 0,
        }
    }

    pub fn active_file(&self) -> Option<&WorkspaceFile> {
        self.files.get(self.active).or_else(|| self.files.first())
    }
}

pub fn default_path() -> PathBuf {
    config_dir().join("workspace.json")
}

pub fn save_to(path: &Path, workspace: &Workspace) -> Result<(), StoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(workspace)?;
    fs::write(path, text)?;
    Ok(())
}

pub fn load_from(path: &Path) -> Result<Workspace, StoreError> {
    let text = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

pub fn save_default(workspace: &Workspace) -> Result<(), StoreError> {
    save_to(&default_path(), workspace)
}

pub fn load_default() -> Result<Workspace, StoreError> {
    load_from(&default_path())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_workspace() {
        let path = std::env::temp_dir().join(format!("sqlt-workspace-{}.json", std::process::id()));
        let workspace = Workspace::new("query.sql", "select 1 from dual");
        save_to(&path, &workspace).unwrap();
        assert_eq!(load_from(&path).unwrap(), workspace);
        assert_eq!(
            workspace.active_file().map(|file| file.sql.as_str()),
            Some("select 1 from dual")
        );
        let _ = fs::remove_file(path);
    }
}
