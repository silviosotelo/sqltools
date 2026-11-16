// SPDX-License-Identifier: GPL-3.0-only
//! Formatter profiles stored as JSON under the app config directory.

use std::fs;
use std::path::{Path, PathBuf};

use sqlt_core::format::FormatOptions;

use crate::store::{config_dir, StoreError};

pub fn profiles_dir() -> PathBuf {
    config_dir().join("profiles")
}

fn profile_path(dir: &Path, name: &str) -> Result<PathBuf, StoreError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.contains(['/', '\\']) || trimmed.contains("..") {
        return Err(StoreError::Invalid(format!("Invalid profile name: {name}")));
    }
    Ok(dir.join(format!("{trimmed}.json")))
}

pub fn save_to(dir: &Path, name: &str, options: &FormatOptions) -> Result<(), StoreError> {
    let path = profile_path(dir, name)?;
    fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(options)?;
    fs::write(path, text)?;
    Ok(())
}

pub fn load_from(dir: &Path, name: &str) -> Result<FormatOptions, StoreError> {
    let path = profile_path(dir, name)?;
    let text = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

pub fn names_in(dir: &Path) -> Result<Vec<String>, StoreError> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
            names.push(stem.to_string());
        }
    }
    names.sort();
    Ok(names)
}

pub fn delete_from(dir: &Path, name: &str) -> Result<(), StoreError> {
    let path = profile_path(dir, name)?;
    if !path.is_file() {
        return Err(StoreError::NotFound(format!("Profile {name} not found")));
    }
    fs::remove_file(path)?;
    Ok(())
}

pub fn save_profile(name: &str, options: &FormatOptions) -> Result<(), StoreError> {
    save_to(&profiles_dir(), name, options)
}

pub fn load_profile(name: &str) -> Result<FormatOptions, StoreError> {
    load_from(&profiles_dir(), name)
}

pub fn profile_names() -> Result<Vec<String>, StoreError> {
    names_in(&profiles_dir())
}

pub fn delete_profile(name: &str) -> Result<(), StoreError> {
    delete_from(&profiles_dir(), name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlt_core::format::{FormatStyle, KeywordCase};

    #[test]
    fn save_list_load_delete() {
        let dir = std::env::temp_dir().join(format!("sqlt-profiles-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let options = FormatOptions {
            indent: 3,
            style: FormatStyle::River,
            keyword_case: KeywordCase::Lower,
            ..FormatOptions::default()
        };
        save_to(&dir, "river", &options).unwrap();
        assert_eq!(names_in(&dir).unwrap(), vec!["river".to_string()]);
        assert_eq!(load_from(&dir, "river").unwrap(), options);
        assert!(save_to(&dir, "../escape", &options).is_err());
        assert!(save_to(&dir, "", &options).is_err());
        delete_from(&dir, "river").unwrap();
        assert!(names_in(&dir).unwrap().is_empty());
        assert!(delete_from(&dir, "river").is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
