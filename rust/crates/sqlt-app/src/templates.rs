// SPDX-License-Identifier: GPL-3.0-only
//! SQL templates stored as JSON under the app config directory.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::store::{config_dir, StoreError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Template {
    pub name: String,
    pub body: String,
}

fn templates_path(dir: &Path) -> PathBuf {
    dir.join("templates.json")
}

fn valid_name(name: &str) -> Result<&str, StoreError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(StoreError::Invalid("Template name is required".to_string()));
    }
    Ok(trimmed)
}

pub fn load_all_from(dir: &Path) -> Result<Vec<Template>, StoreError> {
    let path = templates_path(dir);
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(path)?;
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_str(&text)?)
}

fn save_all_to(dir: &Path, templates: &[Template]) -> Result<(), StoreError> {
    fs::create_dir_all(dir)?;
    let text = serde_json::to_string_pretty(templates)?;
    let tmp = templates_path(dir).with_extension("json.tmp");
    fs::write(&tmp, text)?;
    fs::rename(tmp, templates_path(dir))?;
    Ok(())
}

pub fn save_to(dir: &Path, name: &str, body: &str) -> Result<(), StoreError> {
    let name = valid_name(name)?;
    let mut templates = load_all_from(dir)?;
    match templates.iter_mut().find(|template| template.name == name) {
        Some(existing) => existing.body = body.to_string(),
        None => templates.push(Template {
            name: name.to_string(),
            body: body.to_string(),
        }),
    }
    templates.sort_by(|left, right| left.name.cmp(&right.name));
    save_all_to(dir, &templates)
}

pub fn find_in(dir: &Path, name: &str) -> Result<Option<Template>, StoreError> {
    let name = valid_name(name)?;
    Ok(load_all_from(dir)?
        .into_iter()
        .find(|template| template.name == name))
}

pub fn names_in(dir: &Path) -> Result<Vec<String>, StoreError> {
    Ok(load_all_from(dir)?
        .into_iter()
        .map(|template| template.name)
        .collect())
}

pub fn delete_from(dir: &Path, name: &str) -> Result<(), StoreError> {
    let name = valid_name(name)?;
    let mut templates = load_all_from(dir)?;
    let before = templates.len();
    templates.retain(|template| template.name != name);
    if templates.len() == before {
        return Err(StoreError::NotFound(format!("Template {name} not found")));
    }
    save_all_to(dir, &templates)
}

pub fn list_templates() -> Result<Vec<String>, StoreError> {
    names_in(&config_dir())
}

pub fn load_template(name: &str) -> Result<Option<Template>, StoreError> {
    find_in(&config_dir(), name)
}

pub fn save_template(name: &str, body: &str) -> Result<(), StoreError> {
    save_to(&config_dir(), name, body)
}

pub fn delete_template(name: &str) -> Result<(), StoreError> {
    delete_from(&config_dir(), name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_list_load_delete() {
        let dir = std::env::temp_dir().join(format!("sqlt-templates-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        save_to(&dir, "query", "select 1 from dual").unwrap();
        save_to(&dir, "block", "begin null; end;").unwrap();
        assert_eq!(
            names_in(&dir).unwrap(),
            vec!["block".to_string(), "query".to_string()]
        );
        assert_eq!(
            find_in(&dir, "query")
                .unwrap()
                .map(|template| template.body),
            Some("select 1 from dual".to_string())
        );
        save_to(&dir, "query", "select 2 from dual").unwrap();
        assert_eq!(
            find_in(&dir, "query")
                .unwrap()
                .map(|template| template.body),
            Some("select 2 from dual".to_string())
        );
        assert!(save_to(&dir, "  ", "x").is_err());
        delete_from(&dir, "query").unwrap();
        assert_eq!(names_in(&dir).unwrap(), vec!["block".to_string()]);
        assert!(delete_from(&dir, "query").is_err());
        let _ = fs::remove_dir_all(dir);
    }
}
