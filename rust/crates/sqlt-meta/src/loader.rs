// SPDX-License-Identifier: GPL-3.0-only
//! Oracle loader for the first table/trigger MetaDictionary slice.

use sqlt_core::Value;
use sqlt_oracle::{QueryOptions, Session};

use crate::{TabColumn, Table, WriteSettings};

fn cell(row: &[Value], index: usize) -> String {
    row.get(index)
        .and_then(Value::display)
        .unwrap_or_default()
        .to_string()
}

fn number(row: &[Value], index: usize) -> i64 {
    cell(row, index).parse::<i64>().unwrap_or(0)
}

fn fetch_rows(
    session: &Session,
    sql: &str,
    params: &[&dyn sqlt_oracle::ToSql],
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

pub fn current_schema(session: &Session, options: QueryOptions) -> Result<String, String> {
    let rows = fetch_rows(session, "select user from dual", &[], options)?;
    Ok(rows.first().map(|row| cell(row, 0)).unwrap_or_default())
}

pub fn load_table(
    session: &Session,
    schema: &str,
    name: &str,
    options: QueryOptions,
) -> Result<Table, String> {
    let columns = fetch_rows(
        session,
        "select column_name, data_type, data_length, data_precision, data_scale, \
         nullable, data_default from all_tab_columns \
         where owner = :1 and table_name = :2 order by column_id",
        &[&schema, &name],
        options,
    )?;
    let mut table = Table::new(schema, name);
    for row in columns {
        let precision = number(&row, 3);
        let scale = number(&row, 4);
        let data_default = cell(&row, 6).trim().to_string();
        table.columns.push(TabColumn {
            name: cell(&row, 0),
            data_type: cell(&row, 1),
            data_length: number(&row, 2),
            precision,
            scale,
            nullable: cell(&row, 5) == "Y",
            data_default,
        });
    }
    let storage = fetch_rows(
        session,
        "select tablespace_name, next_extent from all_tables \
         where owner = :1 and table_name = :2",
        &[&schema, &name],
        options,
    )?;
    if let Some(row) = storage.first() {
        let tablespace = cell(row, 0);
        if !tablespace.is_empty() {
            table.tablespace = Some(tablespace);
        }
        let next = number(row, 1);
        if next > 0 {
            table.next_extent = Some(next);
        }
    }
    Ok(table)
}

pub fn load_table_ddl(
    session: &Session,
    schema: Option<&str>,
    name: &str,
    options: QueryOptions,
) -> Result<String, String> {
    let owner = match schema.map(str::trim).filter(|value| !value.is_empty()) {
        Some(owner) => owner.to_string(),
        None => current_schema(session, options)?,
    };
    let table = load_table(session, &owner, name, options)?;
    if table.columns.is_empty() {
        return Err(format!(
            "Table {owner}.{name} has no columns or does not exist"
        ));
    }
    Ok(table.write_definition(&WriteSettings::default()))
}
