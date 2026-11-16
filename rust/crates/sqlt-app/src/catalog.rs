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
