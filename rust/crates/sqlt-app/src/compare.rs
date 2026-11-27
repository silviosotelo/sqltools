// SPDX-License-Identifier: GPL-3.0-only
//! Compare User Objects against a saved snapshot, and To-Do items found in the sources.
//!
//! `sqlt-meta::compare` is a line diff used to validate the MetaDictionary port; this module builds the
//! comparison a developer needs: which objects are missing, which differ and which are identical, with the
//! script that would bring the database to the snapshot.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::browser::ddl_ident;

/// One object in a snapshot: its type and the DDL that was exported.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotObject {
    pub name: String,
    pub object_type: String,
    pub ddl: String,
}

impl SnapshotObject {
    /// PACKAGE and PACKAGE BODY share a name: the type has to be part of the identity.
    pub fn key(&self) -> String {
        format!("{}:{}", self.object_type, self.name)
    }
}

/// `name` -> object. Ordered so a saved file is stable and diffable.
pub type Snapshot = BTreeMap<String, SnapshotObject>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CompareStatus {
    /// Present in both with the same DDL.
    Identical,
    /// Present in both with different DDL.
    Different,
    /// In the snapshot but not in the database.
    Missing,
    /// In the database but not in the snapshot.
    Extra,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareResult {
    pub entries: Vec<CompareEntry>,
    /// Script that brings the database to the snapshot (only what differs or is missing).
    pub script: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareEntry {
    pub name: String,
    pub object_type: String,
    pub status: CompareStatus,
    /// First line that differs, 1-based (0 when there is none).
    pub first_difference: usize,
    /// Lines that differ, for the summary.
    pub differing_lines: usize,
}

/// Compares the snapshot (what should be there) with what the database has now.
pub fn compare_snapshot(snapshot: &Snapshot, current: &Snapshot) -> Vec<CompareEntry> {
    let mut names: Vec<&String> = snapshot.keys().chain(current.keys()).collect();
    names.sort();
    names.dedup();
    let mut entries = Vec::new();
    for name in names {
        let wanted = snapshot.get(name);
        let have = current.get(name);
        let (status, first, differing) = match (wanted, have) {
            (Some(wanted), Some(have)) => {
                let diff = sqlt_meta::compare_ddl(&wanted.ddl, &have.ddl);
                if diff.differing_lines == 0 {
                    (CompareStatus::Identical, 0, 0)
                } else {
                    (
                        CompareStatus::Different,
                        diff.first_difference.unwrap_or(0),
                        diff.differing_lines,
                    )
                }
            }
            (Some(_), None) => (CompareStatus::Missing, 0, 0),
            (None, Some(_)) => (CompareStatus::Extra, 0, 0),
            (None, None) => continue,
        };
        let object = wanted.or(have);
        entries.push(CompareEntry {
            name: object
                .map(|entry| entry.name.clone())
                .unwrap_or_else(|| name.clone()),
            object_type: object
                .map(|entry| entry.object_type.clone())
                .unwrap_or_default(),
            status,
            first_difference: first,
            differing_lines: differing,
        });
    }
    entries
}

/// A specification has to exist before its body, so the script is not a plain alphabetical sort.
fn script_rank(object_type: &str) -> u8 {
    match object_type.trim().to_ascii_uppercase().as_str() {
        "TYPE" => 0,
        "PACKAGE" => 1,
        "TYPE BODY" => 2,
        "PACKAGE BODY" => 3,
        _ => 1,
    }
}

/// `ALL_SOURCE` starts at the unit keyword (`procedure p is`), so the owner goes right after it: running the
/// script from a session whose current schema is another one would otherwise recreate the object there.
fn qualify_header(body: &str, owner: &str, name: &str) -> String {
    let line_end = body.find('\n').unwrap_or(body.len());
    let first = &body[..line_end];
    let keyword_end = first.find(char::is_whitespace).unwrap_or(first.len());
    let haystack = first[keyword_end..].to_ascii_uppercase();
    let Some(offset) = haystack.find(&name.to_ascii_uppercase()) else {
        return body.to_string();
    };
    let start = keyword_end + offset;
    let end = start + name.len();
    let word = |ch: char| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$' | '#');
    let bounded = first[..start]
        .chars()
        .next_back()
        .is_none_or(|ch| !word(ch))
        && first[end..].chars().next().is_none_or(|ch| !word(ch));
    if !bounded {
        return body.to_string();
    }
    let (Ok(owner), Ok(name)) = (ddl_ident(owner), ddl_ident(name)) else {
        return body.to_string();
    };
    format!("{}{owner}.{name}{}", &body[..start], &body[end..])
}

/// The script that makes the database match the snapshot: DDL of every object that differs or is missing,
/// wrapped in a transaction-less header with the object name, so it can be reviewed before running.
pub fn apply_script(snapshot: &Snapshot, entries: &[CompareEntry], owner: Option<&str>) -> String {
    let mut script = String::from("-- Generated by SQLTools: bring the database to the snapshot\n");
    let mut ordered: Vec<&CompareEntry> = entries
        .iter()
        .filter(|entry| {
            entry.status == CompareStatus::Different || entry.status == CompareStatus::Missing
        })
        .collect();
    ordered.sort_by_key(|entry| (script_rank(&entry.object_type), entry.name.clone()));
    for entry in ordered {
        let key = format!("{}:{}", entry.object_type, entry.name);
        let Some(object) = snapshot.get(&key) else {
            continue;
        };
        // ALL_SOURCE stores the unit without its header command: without this prefix the script is not valid.
        let body = object.ddl.trim_end();
        let body = match owner {
            Some(owner) => qualify_header(body, owner, &object.name),
            None => body.to_string(),
        };
        let statement = if body.to_ascii_lowercase().starts_with("create") {
            body
        } else {
            format!("create or replace {body}")
        };
        script.push_str(&format!(
            "\n-- {} ({})\n{}\n",
            entry.name, entry.object_type, statement
        ));
        if !object.ddl.trim_end().ends_with(';') {
            script.push_str(";\n");
        }
        script.push_str("/\n");
    }
    script
}

/// One `TODO`/`FIXME` found in a source line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub owner: String,
    pub name: String,
    pub object_type: String,
    pub line: u32,
    pub text: String,
    /// `TODO` or `FIXME`.
    pub marker: String,
}

/// Finds the markers in one body of source. The marker has to be a whole word: "todos los usuarios" is
/// Spanish, not a note, and `todo_count` is an identifier. A marker inside a string literal is not a note.
pub fn markers_in_line(line: &str) -> Option<String> {
    fn word_char(ch: char) -> bool {
        ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$' | '#')
    }
    // `-- todo` is the same note as `-- TODO`; ASCII upper-casing keeps every byte offset intact.
    let upper = line.to_ascii_uppercase();
    for marker in ["FIXME", "TODO"] {
        let mut from = 0;
        while let Some(offset) = upper[from..].find(marker) {
            let start = from + offset;
            let end = start + marker.len();
            let before = line[..start].chars().next_back();
            let after = line[end..].chars().next();
            let bounded =
                before.is_none_or(|ch| !word_char(ch)) && after.is_none_or(|ch| !word_char(ch));
            let quoted = line[..start].matches('\'').count() % 2 == 1;
            // The marker has to open the comment: between the last `--`/`/*` and the marker only
            // punctuation is allowed, so prose like "SE COMENTO TODO Y SE PASO" is not a note.
            let before = &line[..start];
            let opener = before.rfind("--").max(before.rfind("/*"));
            let after_opener = match opener {
                Some(index) => &before[index + 2..],
                None => before,
            };
            let opens_comment = after_opener
                .chars()
                .all(|ch| ch.is_whitespace() || matches!(ch, '-' | '*'));
            if bounded && !quoted && opens_comment {
                return Some(marker.to_string());
            }
            from = end;
        }
    }
    None
}

/// Keeps only the source lines that carry a marker, with their line number.
pub fn todos_from_source<'a>(
    owner: &str,
    name: &str,
    object_type: &str,
    lines: impl Iterator<Item = (u32, &'a str)>,
) -> Vec<TodoItem> {
    let mut items = Vec::new();
    for (line, text) in lines {
        if let Some(marker) = markers_in_line(text) {
            items.push(TodoItem {
                owner: owner.to_string(),
                name: name.to_string(),
                object_type: object_type.to_string(),
                line,
                text: text.trim().to_string(),
                marker,
            });
        }
    }
    items
}

/// Source text of every program unit of the schema, keyed by `TYPE:NAME`. One query: ALL_SOURCE holds the
/// body, so the comparison is fast even on a big schema (no DDL per object).
pub fn collect_sources(
    session: &sqlt_oracle::Session,
    schema: Option<&str>,
    options: sqlt_oracle::QueryOptions,
) -> Result<Snapshot, String> {
    let sql = if schema.is_some() {
        "select name, type, line, text from all_source where owner = :1 \
         and type in ('PACKAGE','PACKAGE BODY','PROCEDURE','FUNCTION','TRIGGER','TYPE','TYPE BODY','JAVA SOURCE') \
         order by type, name, line"
    } else {
        "select name, type, line, text from user_source \
         and_placeholder order by type, name, line"
    };
    let sql = sql.replace(" and_placeholder", "");
    let owner = schema.map(str::to_string);
    let bindings: Vec<&dyn sqlt_oracle::ToSql> = owner
        .iter()
        .map(|value| value as &dyn sqlt_oracle::ToSql)
        .collect();
    let rows = crate::catalog::fetch_rows(session, &sql, &bindings, options)?;
    let mut snapshot = Snapshot::new();
    for row in &rows {
        let name = crate::catalog::cell(row, 0);
        let object_type = crate::catalog::cell(row, 1);
        let text = crate::catalog::cell(row, 3);
        let object = SnapshotObject {
            name: name.clone(),
            object_type: object_type.clone(),
            ddl: String::new(),
        };
        let entry = snapshot
            .entry(object.key())
            .or_insert_with(|| SnapshotObject {
                ddl: String::new(),
                ..object
            });
        entry.ddl.push_str(&text);
    }
    Ok(snapshot)
}

/// Every line of the schema's sources that carries a `TODO` or `FIXME`, newest last.
pub fn scan_todos(
    session: &sqlt_oracle::Session,
    schema: Option<&str>,
    options: sqlt_oracle::QueryOptions,
) -> Result<Vec<TodoItem>, String> {
    let sql = if schema.is_some() {
        "select owner, name, type, line, text from all_source where owner = :1 \
         and (upper(text) like '%TODO%' or upper(text) like '%FIXME%') order by name, type, line"
    } else {
        "select user, name, type, line, text from user_source \
         where (upper(text) like '%TODO%' or upper(text) like '%FIXME%') order by name, type, line"
    };
    let owner = schema.map(str::to_string);
    let bindings: Vec<&dyn sqlt_oracle::ToSql> = owner
        .iter()
        .map(|value| value as &dyn sqlt_oracle::ToSql)
        .collect();
    let rows = crate::catalog::fetch_rows(session, sql, &bindings, options)?;
    Ok(rows
        .iter()
        .filter_map(|row| {
            let owner = crate::catalog::cell(row, 0);
            let name = crate::catalog::cell(row, 1);
            let object_type = crate::catalog::cell(row, 2);
            let line: u32 = crate::catalog::cell(row, 3).parse().unwrap_or(0);
            let text = crate::catalog::cell(row, 4);
            markers_in_line(&text).map(|marker| TodoItem {
                owner,
                name,
                object_type,
                line,
                text: text.trim().to_string(),
                marker,
            })
        })
        .collect())
}

/// Writes the snapshot as JSON (stable order, so two files can be compared with any diff tool).
pub fn save_snapshot_file(path: &std::path::Path, snapshot: &Snapshot) -> Result<(), String> {
    let json = serde_json::to_string_pretty(snapshot).map_err(|error| error.to_string())?;
    std::fs::write(path, json).map_err(|error| error.to_string())
}

pub fn load_snapshot_file(path: &std::path::Path) -> Result<Snapshot, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(name: &str, kind: &str, ddl: &str) -> SnapshotObject {
        SnapshotObject {
            name: name.to_string(),
            object_type: kind.to_string(),
            ddl: ddl.to_string(),
        }
    }

    /// Keyed the same way the live collection keys it: `TYPE:NAME`.
    fn snapshot(pairs: &[(&str, SnapshotObject)]) -> Snapshot {
        pairs
            .iter()
            .map(|(_, object)| (object.key(), object.clone()))
            .collect()
    }

    #[test]
    fn classifies_identical_different_missing_and_extra() {
        let wanted = snapshot(&[
            ("A", object("A", "TABLE", "create table a (n number)")),
            (
                "B",
                object("B", "VIEW", "create view b as select 1 x from dual"),
            ),
            ("C", object("C", "PACKAGE", "create package c as end")),
        ]);
        let current = snapshot(&[
            ("A", object("A", "TABLE", "create table a (n number)")),
            (
                "B",
                object("B", "VIEW", "create view b as select 2 x from dual"),
            ),
            ("D", object("D", "TABLE", "create table d (n number)")),
        ]);
        let entries = compare_snapshot(&wanted, &current);
        let by_name: BTreeMap<&str, CompareStatus> = entries
            .iter()
            .map(|entry| (entry.name.as_str(), entry.status))
            .collect();
        assert_eq!(by_name["A"], CompareStatus::Identical);
        assert_eq!(by_name["B"], CompareStatus::Different);
        assert_eq!(by_name["C"], CompareStatus::Missing);
        assert_eq!(by_name["D"], CompareStatus::Extra);
        let b = entries.iter().find(|entry| entry.name == "B").unwrap();
        assert_eq!(b.first_difference, 1);
    }

    #[test]
    fn the_script_prefixes_the_unit_source_so_it_can_run() {
        let wanted = snapshot(&[(
            "P",
            object("P", "PROCEDURE", "procedure p is\nbegin\n  null;\nend;"),
        )]);
        let entries = compare_snapshot(&wanted, &Snapshot::new());
        let script = apply_script(&wanted, &entries, None);
        assert!(
            script.contains("create or replace procedure p is"),
            "{script}"
        );

        let already = snapshot(&[(
            "Q",
            object(
                "Q",
                "PROCEDURE",
                "create or replace procedure q is\nbegin null; end;",
            ),
        )]);
        let entries = compare_snapshot(&already, &Snapshot::new());
        let script = apply_script(&already, &entries, None);
        assert!(
            !script.contains("create or replace create or replace"),
            "{script}"
        );
    }

    #[test]
    fn the_script_only_contains_what_has_to_change() {
        let wanted = snapshot(&[
            ("A", object("A", "TABLE", "create table a (n number)")),
            (
                "B",
                object("B", "VIEW", "create view b as select 1 x from dual"),
            ),
            ("C", object("C", "PACKAGE", "create package c as end")),
        ]);
        let current = snapshot(&[("A", object("A", "TABLE", "create table a (n number)"))]);
        let entries = compare_snapshot(&wanted, &current);
        let script = apply_script(&wanted, &entries, None);
        assert!(!script.contains("create table a"));
        assert!(script.contains("create view b"));
        assert!(script.contains("create package c"));
        assert!(script.contains("-- C (PACKAGE)"));
    }

    #[test]
    fn specifications_come_before_their_bodies() {
        let wanted = snapshot(&[
            (
                "P BODY",
                object("P", "PACKAGE BODY", "package body p is\nend p;"),
            ),
            ("P", object("P", "PACKAGE", "package p is\nend p;")),
            ("T", object("T", "TYPE", "type t is object (n number);")),
            ("T BODY", object("T", "TYPE BODY", "type body t is\nend;")),
        ]);
        let entries = compare_snapshot(&wanted, &Snapshot::new());
        let script = apply_script(&wanted, &entries, None);
        let package = script.find("package p is").unwrap();
        let package_body = script.find("package body p is").unwrap();
        let kind = script.find("type t is").unwrap();
        let kind_body = script.find("type body t is").unwrap();
        assert!(package < package_body, "{script}");
        assert!(kind < kind_body, "{script}");
    }

    #[test]
    fn the_script_qualifies_the_object_with_the_compared_owner() {
        let wanted = snapshot(&[(
            "P",
            object("P", "PROCEDURE", "procedure p is\nbegin null; end;"),
        )]);
        let entries = compare_snapshot(&wanted, &Snapshot::new());
        let script = apply_script(&wanted, &entries, Some("ANAMNESIS"));
        assert!(
            script.contains("create or replace procedure \"ANAMNESIS\".\"P\" is"),
            "{script}"
        );
        // The name inside the body is left alone.
        assert!(script.contains("end;"), "{script}");
    }

    #[test]
    fn markers_are_found_but_not_inside_strings() {
        assert_eq!(
            markers_in_line("-- TODO: arreglar"),
            Some("TODO".to_string())
        );
        assert_eq!(
            markers_in_line("x := 1; -- FIXME"),
            Some("FIXME".to_string())
        );
        assert_eq!(markers_in_line("select 'TODO' from dual"), None);
        assert_eq!(markers_in_line("null;"), None);
        assert_eq!(
            markers_in_line("-- todo: pendiente"),
            Some("TODO".to_string())
        );
        assert_eq!(
            markers_in_line("x := 1; -- fixme"),
            Some("FIXME".to_string())
        );
    }

    #[test]
    fn todos_keep_their_line_number() {
        let source = "begin\n  null; -- TODO: revisar\nend;";
        let items = todos_from_source(
            "O",
            "P",
            "PROCEDURE",
            source
                .lines()
                .enumerate()
                .map(|(index, line)| (index as u32 + 1, line)),
        );
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].line, 2);
        assert_eq!(items[0].marker, "TODO");
        assert!(items[0].text.contains("revisar"));
    }
}
