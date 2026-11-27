// SPDX-License-Identifier: GPL-3.0-only
//! Projects: a set of source buffers, the schema and options they were worked with, the objects that were
//! open and free-form notes. Stored as one JSON file per project under `<config>/projects`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::store::config_dir;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFile {
    pub title: String,
    pub sql: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub name: String,
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub files: Vec<ProjectFile>,
    /// `TYPE name` of the objects that were open when the project was saved.
    #[serde(default)]
    pub objects: Vec<String>,
}

pub fn projects_dir() -> PathBuf {
    config_dir().join("projects")
}

/// A file name the projects directory accepts: no separators, no traversal.
pub fn project_file_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    let valid = !trimmed.is_empty()
        && trimmed.chars().count() <= 80
        && !trimmed.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
        && trimmed != "."
        && trimmed != "..";
    if !valid {
        return Err(format!("Invalid project name: {name}"));
    }
    Ok(format!("{trimmed}.json"))
}

/// Every readable project, by name. A file that cannot be parsed is skipped instead of failing the list.
pub fn list(dir: &Path) -> Result<Vec<Project>, String> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut projects = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(project) = serde_json::from_str::<Project>(&text) {
                projects.push(project);
            }
        }
    }
    projects.sort_by_key(|project| project.name.to_lowercase());
    Ok(projects)
}

pub fn save(dir: &Path, project: &Project) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let path = dir.join(project_file_name(&project.name)?);
    let json = serde_json::to_string_pretty(project).map_err(|error| error.to_string())?;
    // A crash half-way through a direct write would truncate a project the user already had: write a
    // sibling first and replace the file in one step.
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, json).map_err(|error| error.to_string())?;
    std::fs::rename(&temp, &path).map_err(|error| error.to_string())
}

pub fn load(dir: &Path, name: &str) -> Result<Project, String> {
    let path = dir.join(project_file_name(name)?);
    let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

pub fn remove(dir: &Path, name: &str) -> Result<(), String> {
    let path = dir.join(project_file_name(name)?);
    std::fs::remove_file(path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(name: &str) -> Project {
        Project {
            name: name.to_string(),
            schema: Some("ANAMNESIS".to_string()),
            notes: "notas".to_string(),
            files: vec![ProjectFile {
                title: "SQL".to_string(),
                sql: "select 1 from dual".to_string(),
            }],
            objects: vec!["PACKAGE AS_PDF3_MOD".to_string()],
        }
    }

    #[test]
    fn round_trip_keeps_every_part() {
        let dir = std::env::temp_dir().join(format!("sqlt-projects-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let original = project("Ventas");
        save(&dir, &original).unwrap();
        assert_eq!(load(&dir, "Ventas").unwrap(), original);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_is_sorted_and_skips_broken_files() {
        let dir = std::env::temp_dir().join(format!("sqlt-projects-list-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        save(&dir, &project("b")).unwrap();
        save(&dir, &project("A")).unwrap();
        std::fs::write(dir.join("roto.json"), "{ this is not json").unwrap();
        let names: Vec<String> = list(&dir).unwrap().into_iter().map(|p| p.name).collect();
        assert_eq!(names, vec!["A", "b"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn names_cannot_escape_the_directory() {
        assert_eq!(project_file_name(" Proyecto ").unwrap(), "Proyecto.json");
        for bad in ["", "..", "../x", "a/b", "a\\b", "a:b"] {
            assert!(project_file_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_project_without_optional_fields_still_loads() {
        let dir = std::env::temp_dir().join(format!("sqlt-projects-min-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Min.json"), r#"{"name":"Min"}"#).unwrap();
        let loaded = load(&dir, "Min").unwrap();
        assert!(loaded.files.is_empty() && loaded.notes.is_empty() && loaded.schema.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
