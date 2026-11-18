// SPDX-License-Identifier: GPL-3.0-only
//! Oracle catalog queries used by the object explorer, Object Viewer and DDL.

use serde::Serialize;
use sqlt_core::Value;
use sqlt_oracle::{QueryOptions, Session, ToSql};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectInfo {
    pub name: String,
    pub object_type: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub data_default: Option<String>,
}

fn cell(row: &[Value], index: usize) -> String {
    row.get(index)
        .and_then(Value::display)
        .unwrap_or_default()
        .to_string()
}

fn fetch_rows(
    session: &Session,
    sql: &str,
    params: &[&dyn ToSql],
    options: QueryOptions,
) -> Result<Vec<Vec<Value>>, String> {
    let mut query = session
        .query_with(sql, params, options)
        .map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    loop {
        let batch = query.next_batch(500).map_err(|e| e.to_string())?;
        if batch.rows.is_empty() {
            break;
        }
        rows.extend(batch.rows);
    }
    Ok(rows)
}

pub fn list_table_names(session: &Session, options: QueryOptions) -> Result<Vec<String>, String> {
    let rows = fetch_rows(
        session,
        "select object_name from user_objects \
         where object_type in ('TABLE', 'VIEW') and rownum <= 1000 \
         order by object_name",
        &[],
        options,
    )?;
    Ok(rows.iter().map(|row| cell(row, 0)).collect())
}

pub fn list_objects(
    session: &Session,
    schema: Option<&str>,
    object_type: Option<&str>,
    options: QueryOptions,
) -> Result<Vec<ObjectInfo>, String> {
    let mut sql = if schema.is_some() {
        "select object_name, object_type, status from all_objects where owner = :1".to_string()
    } else {
        "select object_name, object_type, status from user_objects where 1 = 1".to_string()
    };
    let mut owned: Vec<String> = Vec::new();
    if let Some(owner) = schema {
        owned.push(owner.to_string());
    }
    if let Some(kind) = object_type {
        let index = owned.len() + 1;
        sql.push_str(&format!(" and object_type = :{index}"));
        owned.push(kind.to_string());
    }
    sql.push_str(" order by object_name");
    let bindings: Vec<&dyn ToSql> = owned.iter().map(|value| value as &dyn ToSql).collect();
    let rows = fetch_rows(session, &sql, &bindings, options)?;
    Ok(rows
        .iter()
        .map(|row| ObjectInfo {
            name: cell(row, 0),
            object_type: cell(row, 1),
            status: cell(row, 2),
        })
        .collect())
}

pub fn list_columns(
    session: &Session,
    schema: Option<&str>,
    table: &str,
    options: QueryOptions,
) -> Result<Vec<ColumnInfo>, String> {
    let mut sql = if schema.is_some() {
        "select column_name, data_type, nullable, data_default \
         from all_tab_columns where owner = :1 and table_name = :2"
    } else {
        "select column_name, data_type, nullable, data_default \
         from user_tab_columns where table_name = :1"
    }
    .to_string();
    sql.push_str(" order by column_id");
    let mut owned: Vec<String> = Vec::new();
    if let Some(owner) = schema {
        owned.push(owner.to_string());
    }
    owned.push(table.to_string());
    let bindings: Vec<&dyn ToSql> = owned.iter().map(|value| value as &dyn ToSql).collect();
    let rows = fetch_rows(session, &sql, &bindings, options)?;
    Ok(rows
        .iter()
        .map(|row| ColumnInfo {
            name: cell(row, 0),
            data_type: cell(row, 1),
            nullable: cell(row, 2) == "Y",
            data_default: row
                .get(3)
                .and_then(Value::display)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
        })
        .collect())
}

/// `all_objects.object_type` spells these with a space; `DBMS_METADATA` wants
/// an underscore (`PACKAGE BODY` -> `PACKAGE_BODY`), which `GET_DDL` rejects
/// otherwise with ORA-31600.
fn metadata_object_type(object_type: &str) -> String {
    object_type.trim().to_uppercase().replace(' ', "_")
}

pub fn load_ddl(
    session: &Session,
    schema: Option<&str>,
    name: &str,
    object_type: &str,
    options: QueryOptions,
) -> Result<String, String> {
    let owner = match schema.map(str::trim).filter(|owner| !owner.is_empty()) {
        Some(owner) => owner.to_string(),
        None => {
            let rows = fetch_rows(session, "select user from dual", &[], options)?;
            rows.first().map(|row| cell(row, 0)).unwrap_or_default()
        }
    };
    let object_type = metadata_object_type(object_type);
    let rows = fetch_rows(
        session,
        "select dbms_metadata.get_ddl(:1, :2, :3) from dual",
        &[&object_type, &name, &owner],
        options,
    )?;
    rows.first()
        .map(|row| cell(row, 0))
        .filter(|text| !text.is_empty())
        .ok_or_else(|| "DBMS_METADATA returned an empty DDL".to_string())
}

#[cfg(test)]
mod tests {
    use super::metadata_object_type;

    #[test]
    fn metadata_object_types_use_underscores() {
        assert_eq!(metadata_object_type("TABLE"), "TABLE");
        assert_eq!(metadata_object_type("PACKAGE BODY"), "PACKAGE_BODY");
        assert_eq!(metadata_object_type("TYPE BODY"), "TYPE_BODY");
        assert_eq!(metadata_object_type("DATABASE LINK"), "DATABASE_LINK");
        assert_eq!(metadata_object_type(" package body "), "PACKAGE_BODY");
    }
}

pub fn load_table_ddl_meta(
    session: &Session,
    schema: Option<&str>,
    name: &str,
    options: QueryOptions,
) -> Result<String, String> {
    sqlt_meta::loader::load_table_ddl(session, schema, name, options)
}
/// DDL for the objects matching the explorer filter, up to `limit`.
/// Tables use the ported MetaDictionary path; other objects use
/// `DBMS_METADATA`. Objects that fail are listed as comments in the output.
pub fn extract_schema_ddl(
    session: &Session,
    schema: Option<&str>,
    object_type: Option<&str>,
    limit: usize,
    options: QueryOptions,
) -> Result<(String, usize), String> {
    // Same OCI session as the script runner: serialize the whole extraction so
    // it cannot interleave with a running script.
    let _execution = session.lock_execution();
    if session.is_closed() {
        return Err("The connection was closed".to_string());
    }
    let objects = list_objects(session, schema, object_type, options)?;
    let mut out = String::new();
    let mut done = 0usize;
    for object in objects.iter().take(limit) {
        let ddl = if object.object_type == "TABLE" {
            load_table_ddl_meta(session, schema, &object.name, options)
        } else {
            load_ddl(session, schema, &object.name, &object.object_type, options)
        };
        match ddl {
            Ok(text) => {
                out.push_str(&text);
                if !text.ends_with("\n\n") {
                    out.push('\n');
                }
                done += 1;
            }
            Err(error) => out.push_str(&format!(
                "-- {} ({}): {}\n\n",
                object.name, object.object_type, error
            )),
        }
    }
    Ok((out, done))
}

pub fn session_stats(
    session: &Session,
    options: QueryOptions,
) -> Result<Vec<(String, String)>, String> {
    let rows = fetch_rows(
        session,
        "select sn.name, ms.value from v$mystat ms \
         join v$statname sn on sn.statistic# = ms.statistic# \
         where ms.value > 0 order by ms.value desc",
        &[],
        options,
    )?;
    Ok(rows
        .iter()
        .map(|row| (cell(row, 0), cell(row, 1)))
        .collect())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstraintInfo {
    pub name: String,
    pub kind: String,
    pub status: String,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexInfo {
    pub name: String,
    pub uniqueness: String,
    pub status: String,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerInfo {
    pub name: String,
    pub trigger_type: String,
    pub event: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantInfo {
    pub grantee: String,
    pub privilege: String,
    pub grantable: bool,
}

struct TableScope {
    owner: Option<String>,
    table: String,
}

impl TableScope {
    fn new(schema: Option<&str>, table: &str) -> Self {
        TableScope {
            owner: schema.map(str::to_string),
            table: table.to_string(),
        }
    }

    fn bindings(&self) -> Vec<String> {
        let mut values = Vec::new();
        if let Some(owner) = &self.owner {
            values.push(owner.clone());
        }
        values.push(self.table.clone());
        values
    }

    fn source(&self, all_name: &str, user_name: &str) -> String {
        if self.owner.is_some() {
            all_name.to_string()
        } else {
            user_name.to_string()
        }
    }

    fn where_clause(&self) -> String {
        if self.owner.is_some() {
            "owner = :1 and table_name = :2".to_string()
        } else {
            "table_name = :1".to_string()
        }
    }
}

fn grouped_columns(
    session: &Session,
    sql: &str,
    bindings: &[&dyn ToSql],
    options: QueryOptions,
) -> Result<std::collections::HashMap<String, Vec<String>>, String> {
    let rows = fetch_rows(session, sql, bindings, options)?;
    let mut grouped: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for row in rows {
        grouped
            .entry(cell(&row, 0))
            .or_default()
            .push(cell(&row, 1));
    }
    Ok(grouped)
}

pub fn list_constraints(
    session: &Session,
    schema: Option<&str>,
    table: &str,
    options: QueryOptions,
) -> Result<Vec<ConstraintInfo>, String> {
    let scope = TableScope::new(schema, table);
    let bindings_owned = scope.bindings();
    let bindings: Vec<&dyn ToSql> = bindings_owned
        .iter()
        .map(|value| value as &dyn ToSql)
        .collect();
    let where_clause = scope.where_clause();
    let constraints = fetch_rows(
        session,
        &format!(
            "select constraint_name, constraint_type, status from {} where {} order by constraint_type, constraint_name",
            scope.source("all_constraints", "user_constraints"),
            where_clause
        ),
        &bindings,
        options,
    )?;
    let columns = grouped_columns(
        session,
        &format!(
            "select constraint_name, column_name from {} where {} order by position",
            scope.source("all_cons_columns", "user_cons_columns"),
            where_clause
        ),
        &bindings,
        options,
    )?;
    Ok(constraints
        .iter()
        .map(|row| {
            let name = cell(row, 0);
            ConstraintInfo {
                columns: columns.get(&name).cloned().unwrap_or_default(),
                name,
                kind: cell(row, 1),
                status: cell(row, 2),
            }
        })
        .collect())
}

pub fn list_indexes(
    session: &Session,
    schema: Option<&str>,
    table: &str,
    options: QueryOptions,
) -> Result<Vec<IndexInfo>, String> {
    let scope = TableScope::new(schema, table);
    let bindings_owned = scope.bindings();
    let bindings: Vec<&dyn ToSql> = bindings_owned
        .iter()
        .map(|value| value as &dyn ToSql)
        .collect();
    let where_clause = scope.where_clause();
    let indexes = fetch_rows(
        session,
        &format!(
            "select index_name, uniqueness, status from {} where {} order by index_name",
            scope.source("all_indexes", "user_indexes"),
            where_clause
        ),
        &bindings,
        options,
    )?;
    let columns = grouped_columns(
        session,
        &format!(
            "select index_name, column_name from {} where {} order by column_position",
            scope.source("all_ind_columns", "user_ind_columns"),
            where_clause
        ),
        &bindings,
        options,
    )?;
    Ok(indexes
        .iter()
        .map(|row| {
            let name = cell(row, 0);
            IndexInfo {
                columns: columns.get(&name).cloned().unwrap_or_default(),
                name,
                uniqueness: cell(row, 1),
                status: cell(row, 2),
            }
        })
        .collect())
}

pub fn list_triggers(
    session: &Session,
    schema: Option<&str>,
    table: &str,
    options: QueryOptions,
) -> Result<Vec<TriggerInfo>, String> {
    let scope = TableScope::new(schema, table);
    let bindings_owned = scope.bindings();
    let bindings: Vec<&dyn ToSql> = bindings_owned
        .iter()
        .map(|value| value as &dyn ToSql)
        .collect();
    let rows = fetch_rows(
        session,
        &format!(
            "select trigger_name, trigger_type, triggering_event, status from {} where {} order by trigger_name",
            scope.source("all_triggers", "user_triggers"),
            scope.where_clause()
        ),
        &bindings,
        options,
    )?;
    Ok(rows
        .iter()
        .map(|row| TriggerInfo {
            name: cell(row, 0),
            trigger_type: cell(row, 1),
            event: cell(row, 2),
            status: cell(row, 3),
        })
        .collect())
}

pub fn list_grants(
    session: &Session,
    schema: Option<&str>,
    table: &str,
    options: QueryOptions,
) -> Result<Vec<GrantInfo>, String> {
    let scope = TableScope::new(schema, table);
    let bindings_owned = scope.bindings();
    let bindings: Vec<&dyn ToSql> = bindings_owned
        .iter()
        .map(|value| value as &dyn ToSql)
        .collect();
    let rows = fetch_rows(
        session,
        &format!(
            "select grantee, privilege, grantable from {} where {} order by grantee, privilege",
            scope.source("all_tab_privs", "user_tab_privs"),
            scope.where_clause()
        ),
        &bindings,
        options,
    )?;
    Ok(rows
        .iter()
        .map(|row| GrantInfo {
            grantee: cell(row, 0),
            privilege: cell(row, 1),
            grantable: cell(row, 2) == "YES",
        })
        .collect())
}
