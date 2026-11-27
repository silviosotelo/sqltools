// SPDX-License-Identifier: GPL-3.0-only
//! Saved SQL reports and the docked file browser.
//!
//! A report is a `.sql` file in `<config>/reports`: the parameters are the `&name` / `&&name` substitutions
//! the script runner already understands, so a report needs no new execution path. The file browser lists
//! the user's declared locations (or sensible defaults) and nothing else.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::store::config_dir;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub name: String,
    pub sql: String,
    /// Substitution variables in the order they appear, without duplicates.
    pub params: Vec<String>,
}

/// `&name` and `&&name` in source order, unique. Quoted literals are ignored, like the script runner does.
pub fn parameters(sql: &str) -> Vec<String> {
    let bytes = sql.as_bytes();
    let mut params: Vec<String> = Vec::new();
    let mut i = 0;
    let mut quote: Option<u8> = None;
    while i < bytes.len() {
        let ch = bytes[i];
        if let Some(active) = quote {
            if ch == active {
                quote = None;
            }
            i += 1;
            continue;
        }
        if ch == b'\'' || ch == b'"' {
            quote = Some(ch);
            i += 1;
            continue;
        }
        if ch == b'&' {
            // `&&name` is the same parameter for the user; skip the extra marker.
            let mut start = i + 1;
            if bytes.get(start) == Some(&b'&') {
                start += 1;
            }
            let mut end = start;
            while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
                end += 1;
            }
            if end > start {
                let name = String::from_utf8_lossy(&bytes[start..end]).to_string();
                if !name.is_empty() && !params.iter().any(|known| known.eq_ignore_ascii_case(&name))
                {
                    params.push(name);
                }
                i = end;
                continue;
            }
        }
        i += 1;
    }
    params
}

/// A file name the reports directory accepts: no separators, no traversal.
pub fn report_file_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    let valid = !trimmed.is_empty()
        && trimmed.chars().count() <= 80
        && !trimmed.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
        && trimmed != "."
        && trimmed != "..";
    if !valid {
        return Err(format!("Invalid report name: {name}"));
    }
    Ok(format!("{trimmed}.sql"))
}

pub fn reports_dir() -> PathBuf {
    config_dir().join("reports")
}

pub fn list(dir: &Path) -> Result<Vec<Report>, String> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut reports: Vec<Report> = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("sql") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();
        let sql = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
        reports.push(Report {
            params: parameters(&sql),
            name,
            sql,
        });
    }
    reports.sort_by_key(|report| report.name.to_lowercase());
    Ok(reports)
}

pub fn save(dir: &Path, name: &str, sql: &str) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let file = dir.join(report_file_name(name)?);
    std::fs::write(file, sql).map_err(|error| error.to_string())
}

pub fn remove(dir: &Path, name: &str) -> Result<(), String> {
    let file = dir.join(report_file_name(name)?);
    std::fs::remove_file(file).map_err(|error| error.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

/// Directories first, then files, both by name; dot files are skipped.
pub fn list_dir(path: &Path) -> Result<Vec<FileEntry>, String> {
    let mut entries: Vec<FileEntry> = Vec::new();
    for entry in std::fs::read_dir(path).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let metadata = entry.metadata().map_err(|error| error.to_string())?;
        entries.push(FileEntry {
            name,
            path: entry.path().display().to_string(),
            is_dir: metadata.is_dir(),
            size: if metadata.is_dir() { 0 } else { metadata.len() },
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

/// The locations the browser shows: the ones saved in `locations.json`, or the user's documents.
pub fn locations() -> Vec<String> {
    let file = config_dir().join("locations.json");
    if let Ok(text) = std::fs::read_to_string(&file) {
        if let Ok(list) = serde_json::from_str::<Vec<String>>(&text) {
            if !list.is_empty() {
                return list;
            }
        }
    }
    let mut defaults = Vec::new();
    if let Ok(profile) = std::env::var("USERPROFILE") {
        let documents = PathBuf::from(&profile).join("Documents");
        if documents.is_dir() {
            defaults.push(documents.display().to_string());
        }
        if PathBuf::from(&profile).is_dir() {
            defaults.push(profile);
        }
    }
    defaults
}

pub fn save_locations(list: &[String]) -> Result<(), String> {
    std::fs::create_dir_all(config_dir()).map_err(|error| error.to_string())?;
    let file = config_dir().join("locations.json");
    let json = serde_json::to_string_pretty(list).map_err(|error| error.to_string())?;
    std::fs::write(file, json).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parameters_are_unique_and_ignore_literals() {
        let sql =
            "select * from t where a = &owner and b = '&notaparam' and c = &year or d = &&OWNER";
        assert_eq!(parameters(sql), vec!["owner", "year"]);
        assert_eq!(parameters("select 1 from dual"), Vec::<String>::new());
    }

    #[test]
    fn report_names_cannot_escape_the_directory() {
        assert_eq!(report_file_name(" Ventas ").unwrap(), "Ventas.sql");
        for bad in ["", "  ", "../x", "a/b", "a\\b", "..", "a:b"] {
            assert!(report_file_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn reports_round_trip_and_list_sorted() {
        let dir = std::env::temp_dir().join(format!("sqlt-reports-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        save(&dir, "b", "select * from t where x = &y").unwrap();
        save(&dir, "A", "select 1 from dual").unwrap();
        let reports = list(&dir).unwrap();
        assert_eq!(
            reports
                .iter()
                .map(|report| report.name.as_str())
                .collect::<Vec<_>>(),
            vec!["A", "b"]
        );
        assert_eq!(reports[1].params, vec!["y"]);
        remove(&dir, "b").unwrap();
        assert_eq!(list(&dir).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn directories_come_first() {
        let dir = std::env::temp_dir().join(format!("sqlt-files-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("zeta")).unwrap();
        std::fs::write(dir.join("alpha.sql"), "select 1 from dual").unwrap();
        std::fs::write(dir.join(".hidden"), "x").unwrap();
        let entries = list_dir(&dir).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].is_dir);
        assert_eq!(entries[0].name, "zeta");
        assert_eq!(entries[1].name, "alpha.sql");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
