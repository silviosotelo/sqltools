// SPDX-License-Identifier: GPL-3.0-only
//! Import connection profiles from the C++/Qt SQLTools Mac preferences
//! (`connections/list`, a JSON array inside QSettings). Passwords are never
//! imported: the old app stored them in a different keychain domain.

use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::store::{ConnectionConfig, ConnectionStore, StoreError};

fn string(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn bool_value(value: &Value, key: &str, default: bool) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn mode(value: &Value) -> String {
    match string(value, "mode").to_ascii_uppercase().as_str() {
        "SYSDBA" => "SYSDBA".to_string(),
        "SYSOPER" => "SYSOPER".to_string(),
        _ => "NORMAL".to_string(),
    }
}

pub fn import_legacy_json(path: &Path, store: &mut ConnectionStore) -> Result<usize, StoreError> {
    let text = fs::read_to_string(path)?;
    let root: Value = serde_json::from_str(&text)?;
    let profiles = root
        .as_array()
        .cloned()
        .or_else(|| root.get("connections").and_then(Value::as_array).cloned())
        .ok_or_else(|| {
            StoreError::Invalid(
                "Legacy preferences must contain an array of connections".to_string(),
            )
        })?;

    let mut imported = 0usize;
    for profile in profiles {
        let user = string(&profile, "user");
        if user.is_empty() {
            continue;
        }
        let alias = string(&profile, "alias");
        let bypass = bool_value(&profile, "bypass", false);
        let connect = if bypass {
            let host = string(&profile, "host");
            let port = match string(&profile, "port") {
                port if port.is_empty() => "1521".to_string(),
                port => port,
            };
            let sid = string(&profile, "sid");
            if host.is_empty() || sid.is_empty() {
                None
            } else {
                let service = bool_value(&profile, "useService", true);
                let key = if service { "SERVICE_NAME" } else { "SID" };
                Some(format!(
                    "(DESCRIPTION=(ADDRESS=(PROTOCOL=TCP)(HOST={host})(PORT={port}))(CONNECT_DATA=({key}={sid})))"
                ))
            }
        } else {
            None
        };
        if alias.is_empty() && connect.is_none() {
            continue;
        }
        let tag = string(&profile, "tag");
        let label = if tag.is_empty() {
            format!("{user}@{}", alias)
        } else {
            tag
        };
        let old_id = string(&profile, "id");
        let mut config = ConnectionConfig::new(&label, &user);
        if !old_id.is_empty() {
            config.id = old_id;
        }
        config.alias = if alias.is_empty() { None } else { Some(alias) };
        config.connect = connect;
        let tns_admin = string(&profile, "tnsAdmin");
        config.tns_admin = if tns_admin.is_empty() {
            None
        } else {
            Some(tns_admin)
        };
        config.mode = mode(&profile);
        config.read_only = bool_value(&profile, "readOnly", false);
        config.save_password = false;
        // Never replace a profile that already has this id: it may carry edits (environment, paths).
        let duplicate = store.get(&config.id).is_some()
            || store.list().iter().any(|existing| {
                existing.label == config.label
                    && existing.user == config.user
                    && existing.alias == config.alias
                    && existing.connect == config.connect
            });
        if duplicate {
            continue;
        }
        store.upsert(config)?;
        imported += 1;
    }
    Ok(imported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_legacy_profiles_without_passwords() {
        let dir = std::env::temp_dir().join(format!("sqlt-migrate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let legacy = dir.join("legacy.json");
        let store_path = dir.join("connections.json");
        fs::write(
            &legacy,
            r#"[
              {"id":"old-1","tag":"Legacy","user":"scott","alias":"santa","tnsAdmin":"C:\\tns","bypass":false,"mode":"Normal","readOnly":true,"savePassword":true},
              {"id":"old-2","tag":"Direct","user":"sys","bypass":true,"host":"10.0.0.5","port":"1521","sid":"anam","useService":false,"mode":"SYSDBA"}
            ]"#,
        )
        .unwrap();
        let mut store = ConnectionStore::new(store_path.clone());
        assert_eq!(import_legacy_json(&legacy, &mut store).unwrap(), 2);
        let list = store.list();
        assert_eq!(list[0].label, "Legacy");
        assert_eq!(list[0].alias.as_deref(), Some("santa"));
        assert!(list[0].read_only);
        assert!(!list[0].save_password);
        assert!(list[1]
            .connect
            .as_deref()
            .unwrap_or_default()
            .contains("SID=anam"));
        assert_eq!(list[1].mode, "SYSDBA");
        assert_eq!(import_legacy_json(&legacy, &mut store).unwrap(), 0);
        let text = fs::read_to_string(&store_path).unwrap();
        assert!(!text.contains("\"password\""));
        let _ = fs::remove_dir_all(dir);
    }
}
