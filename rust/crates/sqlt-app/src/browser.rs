// SPDX-License-Identifier: GPL-3.0-only
//! Object Browser extras: dependencies, compile, trigger/constraint state and compile errors.
//! Every statement here builds SQL from identifiers that come from the dictionary, so names are quoted
//! and the object type is checked against a whitelist before it reaches the ALTER statement.

use serde::Serialize;
use sqlt_oracle::{QueryOptions, Session, ToSql};

use crate::catalog::{cell, fetch_rows, ObjectInfo};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub owner: String,
    pub name: String,
    pub object_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileError {
    pub line: u32,
    pub position: u32,
    pub text: String,
}

/// Object types that Oracle lets us compile with `ALTER <type> <name> COMPILE`.
const COMPILABLE: [&str; 8] = [
    "PROCEDURE",
    "FUNCTION",
    "PACKAGE",
    "PACKAGE BODY",
    "TYPE",
    "TYPE BODY",
    "TRIGGER",
    "VIEW",
];

/// Quotes an identifier for DDL. Dictionary names can contain almost anything, so a double quote is
/// refused instead of escaped: no object created by a sane tool has one, and this cannot be injected.
pub fn ddl_ident(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.contains('"') || trimmed.contains('\0') {
        return Err(format!("Unsafe identifier: {name}"));
    }
    Ok(format!("\"{trimmed}\""))
}

fn qualified(owner: Option<&str>, name: &str) -> Result<String, String> {
    let name = ddl_ident(name)?;
    match owner {
        Some(owner) => Ok(format!("{}.{}", ddl_ident(owner)?, name)),
        None => Ok(name),
    }
}

fn compile_type(object_type: &str) -> Result<&'static str, String> {
    let upper = object_type.trim().to_ascii_uppercase();
    COMPILABLE
        .iter()
        .find(|kind| **kind == upper)
        .copied()
        .ok_or_else(|| format!("Object type cannot be compiled: {object_type}"))
}

/// `dependents = false` lists what this object uses; `true` lists what uses it. Level 1 only.
pub fn list_dependencies(
    session: &Session,
    schema: Option<&str>,
    name: &str,
    object_type: &str,
    dependents: bool,
    options: QueryOptions,
) -> Result<Vec<Dependency>, String> {
    let owner = schema.map(str::to_string);
    let bindings_owned: Vec<String> = match &owner {
        Some(owner) => vec![owner.clone(), name.to_string(), object_type.to_string()],
        None => vec![name.to_string(), object_type.to_string()],
    };
    // `all_dependencies` has hundreds of thousands of rows on a real database: always filtered.
    let (sql, columns) = match (&owner, dependents) {
        (Some(_), false) => (
            "select referenced_owner, referenced_name, referenced_type from all_dependencies \
             where owner = :1 and name = :2 and type = :3 and referenced_name is not null \
             order by referenced_owner, referenced_name",
            (0, 1, 2),
        ),
        (Some(_), true) => (
            "select owner, name, type from all_dependencies \
             where referenced_owner = :1 and referenced_name = :2 and referenced_type = :3 \
             order by owner, name",
            (0, 1, 2),
        ),
        (None, false) => (
            "select referenced_owner, referenced_name, referenced_type from user_dependencies \
             where name = :1 and type = :2 and referenced_name is not null \
             order by referenced_owner, referenced_name",
            (0, 1, 2),
        ),
        (None, true) => (
            "select owner, name, type from all_dependencies \
             where referenced_owner = user and referenced_name = :1 and referenced_type = :2 \
             order by owner, name",
            (0, 1, 2),
        ),
    };
    let bindings: Vec<&dyn ToSql> = bindings_owned
        .iter()
        .map(|value| value as &dyn ToSql)
        .collect();
    let rows = fetch_rows(session, sql, &bindings, options)?;
    Ok(rows
        .iter()
        .map(|row| Dependency {
            owner: cell(row, columns.0),
            name: cell(row, columns.1),
            object_type: cell(row, columns.2),
        })
        .collect())
}

/// Objects of the schema whose last compile failed.
pub fn list_invalid_objects(
    session: &Session,
    schema: Option<&str>,
    options: QueryOptions,
) -> Result<Vec<ObjectInfo>, String> {
    let sql = if schema.is_some() {
        "select object_name, object_type, status from all_objects \
         where owner = :1 and status = 'INVALID' order by object_type, object_name"
    } else {
        "select object_name, object_type, status from user_objects \
         where status = 'INVALID' order by object_type, object_name"
    };
    let owner = schema.map(str::to_string);
    let bindings: Vec<&dyn ToSql> = owner.iter().map(|value| value as &dyn ToSql).collect();
    let rows = fetch_rows(session, sql, &bindings, options)?;
    Ok(rows
        .iter()
        .map(|row| ObjectInfo {
            name: cell(row, 0),
            object_type: cell(row, 1),
            status: cell(row, 2),
        })
        .collect())
}

/// Recompiles one unit with `REUSE SETTINGS`, so the session's compile flags are preserved.
pub fn compile_object(
    session: &Session,
    schema: Option<&str>,
    name: &str,
    object_type: &str,
) -> Result<(), String> {
    let kind = compile_type(object_type)?;
    let sql = format!(
        "alter {kind} {} compile reuse settings",
        qualified(schema, name)?
    );
    session
        .execute_with(&sql, &[])
        .map(|_| ())
        .map_err(|error| format!("{error}"))
}

/// Compiles every invalid unit of the schema and returns how many were attempted and how many are still
/// invalid afterwards (a unit can fail again, and its errors are read with [`list_errors`]).
pub fn compile_invalid_objects(
    session: &Session,
    schema: Option<&str>,
    options: QueryOptions,
) -> Result<(usize, usize), String> {
    let invalid = list_invalid_objects(session, schema, options)?;
    let mut attempted = 0usize;
    for object in &invalid {
        if compile_type(&object.object_type).is_err() {
            continue; // LOB, INDEX and friends cannot be compiled; leave them alone
        }
        attempted += 1;
        let _ = compile_object(session, schema, &object.name, &object.object_type);
    }
    let remaining = list_invalid_objects(session, schema, options)?.len();
    Ok((attempted, remaining))
}

/// `ALTER TRIGGER <name> ENABLE|DISABLE`.
pub fn set_trigger_enabled(
    session: &Session,
    schema: Option<&str>,
    name: &str,
    enabled: bool,
) -> Result<(), String> {
    let sql = format!(
        "alter trigger {} {}",
        qualified(schema, name)?,
        if enabled { "enable" } else { "disable" }
    );
    session
        .execute_with(&sql, &[])
        .map(|_| ())
        .map_err(|error| format!("{error}"))
}

/// `ALTER TABLE <table> ENABLE|DISABLE CONSTRAINT <constraint>`.
pub fn set_constraint_enabled(
    session: &Session,
    schema: Option<&str>,
    table: &str,
    constraint: &str,
    enabled: bool,
) -> Result<(), String> {
    let sql = format!(
        "alter table {} {} constraint {}",
        qualified(schema, table)?,
        if enabled { "enable" } else { "disable" },
        ddl_ident(constraint)?
    );
    session
        .execute_with(&sql, &[])
        .map(|_| ())
        .map_err(|error| format!("{error}"))
}

/// Compiler errors of the last compile of one unit, in report order.
pub fn list_errors(
    session: &Session,
    schema: Option<&str>,
    name: &str,
    object_type: &str,
    options: QueryOptions,
) -> Result<Vec<CompileError>, String> {
    let owner = schema.map(str::to_string);
    let mut bindings_owned = vec![name.to_string(), object_type.to_string()];
    let sql = match &owner {
        Some(owner) => {
            bindings_owned.insert(0, owner.clone());
            "select line, position, text from all_errors where owner = :1 and name = :2 and type = :3 \
             order by sequence"
        }
        None => {
            "select line, position, text from user_errors where name = :1 and type = :2 \
             order by sequence"
        }
    };
    let bindings: Vec<&dyn ToSql> = bindings_owned
        .iter()
        .map(|value| value as &dyn ToSql)
        .collect();
    let rows = fetch_rows(session, sql, &bindings, options)?;
    Ok(rows
        .iter()
        .map(|row| CompileError {
            line: cell(row, 0).parse().unwrap_or(0),
            position: cell(row, 1).parse().unwrap_or(0),
            text: cell(row, 2),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_quoted_and_unsafe_ones_refused() {
        assert_eq!(ddl_ident("A11").unwrap(), "\"A11\"");
        assert!(ddl_ident("A\"B").is_err());
        assert!(ddl_ident("").is_err());
        assert!(ddl_ident("   ").is_err());
        assert_eq!(
            qualified(Some("ANAMNESIS"), "A11").unwrap(),
            "\"ANAMNESIS\".\"A11\""
        );
        assert_eq!(qualified(None, "A11").unwrap(), "\"A11\"");
    }

    #[test]
    fn only_compilable_types_are_accepted() {
        assert_eq!(compile_type("package body").unwrap(), "PACKAGE BODY");
        assert_eq!(compile_type("VIEW").unwrap(), "VIEW");
        for rejected in ["TABLE", "INDEX", "LOB", "SYNONYM", "SEQUENCE", ""] {
            assert!(compile_type(rejected).is_err(), "{rejected}");
        }
    }
}
