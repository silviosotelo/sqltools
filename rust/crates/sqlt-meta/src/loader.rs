// SPDX-License-Identifier: GPL-3.0-only
//! Oracle loader for the MetaDictionary table slice: columns, storage,
//! indexes, constraints, triggers, comments and grants.

use std::collections::{BTreeMap, HashMap};

use sqlt_core::Value;
use sqlt_oracle::{QueryOptions, Session};

use crate::{Constraint, Grant, Index, TabColumn, Table, Trigger, WriteSettings};

fn cell(row: &[Value], index: usize) -> String {
    row.get(index)
        .and_then(Value::display)
        .unwrap_or_default()
        .to_string()
}

fn number(row: &[Value], index: usize) -> i64 {
    cell(row, index).parse::<i64>().unwrap_or(0)
}

fn option_cell(row: &[Value], index: usize) -> Option<String> {
    let value = cell(row, index).trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
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

fn load_comments(
    session: &Session,
    schema: &str,
    name: &str,
    options: QueryOptions,
    table: &mut Table,
) -> Result<(), String> {
    let rows = fetch_rows(
        session,
        "select comments from all_tab_comments where owner = :1 and table_name = :2",
        &[&schema, &name],
        options,
    )?;
    if let Some(row) = rows.first() {
        table.comments = option_cell(row, 0);
    }
    let rows = fetch_rows(
        session,
        "select column_name, comments from all_col_comments \
         where owner = :1 and table_name = :2 and comments is not null",
        &[&schema, &name],
        options,
    )?;
    let comments: HashMap<String, String> = rows
        .iter()
        .map(|row| (cell(row, 0), cell(row, 1)))
        .collect();
    for column in &mut table.columns {
        column.comments = comments.get(&column.name).cloned();
    }
    Ok(())
}

fn load_indexes(
    session: &Session,
    schema: &str,
    name: &str,
    options: QueryOptions,
    table: &mut Table,
) -> Result<(), String> {
    let constraint_rows = fetch_rows(
        session,
        "select index_name from all_constraints \
         where owner = :1 and table_name = :2 and index_name is not null",
        &[&schema, &name],
        options,
    )?;
    let constraint_indexes: std::collections::HashSet<String> =
        constraint_rows.iter().map(|row| cell(row, 0)).collect();
    let rows = fetch_rows(
        session,
        "select i.index_name, i.uniqueness, i.index_type, i.tablespace_name, \
         c.column_name from all_indexes i \
         left join all_ind_columns c on c.index_owner = i.owner and c.index_name = i.index_name \
         where i.owner = :1 and i.table_name = :2 \
         order by i.index_name, c.column_position",
        &[&schema, &name],
        options,
    )?;
    let mut by_name: BTreeMap<String, Index> = BTreeMap::new();
    for row in rows {
        let index_name = cell(&row, 0);
        if constraint_indexes.contains(&index_name) {
            continue;
        }
        let entry = by_name.entry(index_name.clone()).or_insert_with(|| Index {
            name: index_name,
            unique: cell(&row, 1) == "UNIQUE",
            bitmap: cell(&row, 2).contains("BITMAP"),
            columns: Vec::new(),
            tablespace: option_cell(&row, 3),
        });
        let column = cell(&row, 4);
        if !column.is_empty() {
            entry.columns.push(column);
        }
    }
    table.indexes = by_name.into_values().collect();
    Ok(())
}

fn load_constraints(
    session: &Session,
    schema: &str,
    name: &str,
    options: QueryOptions,
    table: &mut Table,
) -> Result<(), String> {
    let referenced_rows = fetch_rows(
        session,
        "select c.constraint_name, c.table_name, cc.column_name \
         from all_constraints c \
         join all_cons_columns cc on cc.owner = c.owner and cc.constraint_name = c.constraint_name \
         where c.owner = :1 and c.constraint_type in ('P', 'U') \
         order by c.constraint_name, cc.position",
        &[&schema],
        options,
    )?;
    let mut referenced: HashMap<String, (String, Vec<String>)> = HashMap::new();
    for row in referenced_rows {
        let entry = referenced
            .entry(cell(&row, 0))
            .or_insert_with(|| (cell(&row, 1), Vec::new()));
        entry.1.push(cell(&row, 2));
    }

    let rows = fetch_rows(
        session,
        "select c.constraint_name, c.constraint_type, c.search_condition, \
         c.r_constraint_name, c.delete_rule, cc.column_name \
         from all_constraints c \
         left join all_cons_columns cc on cc.owner = c.owner and cc.constraint_name = c.constraint_name \
         where c.owner = :1 and c.table_name = :2 \
         and c.constraint_type in ('P', 'U', 'R', 'C') \
         order by c.constraint_type, c.constraint_name, cc.position",
        &[&schema, &name],
        options,
    )?;
    let mut by_name: BTreeMap<String, Constraint> = BTreeMap::new();
    for row in rows {
        let constraint_name = cell(&row, 0);
        let kind = cell(&row, 1).chars().next().unwrap_or('C');
        let entry = by_name.entry(constraint_name.clone()).or_insert_with(|| {
            let mut constraint = Constraint {
                name: constraint_name,
                kind,
                columns: Vec::new(),
                condition: option_cell(&row, 2),
                ref_table: None,
                ref_columns: Vec::new(),
                delete_rule: option_cell(&row, 4),
            };
            if kind == 'R' {
                if let Some((ref_table, ref_columns)) = referenced.get(&cell(&row, 3)) {
                    constraint.ref_table = Some(ref_table.clone());
                    constraint.ref_columns = ref_columns.clone();
                }
            }
            constraint
        });
        let column = cell(&row, 5);
        if !column.is_empty() {
            entry.columns.push(column);
        }
    }
    table.constraints = by_name.into_values().collect();
    Ok(())
}

fn load_triggers(
    session: &Session,
    schema: &str,
    name: &str,
    options: QueryOptions,
    table: &mut Table,
) -> Result<(), String> {
    let rows = fetch_rows(
        session,
        "select trigger_name, description, trigger_body from all_triggers \
         where owner = :1 and table_name = :2 and status = 'ENABLED' \
         order by trigger_name",
        &[&schema, &name],
        options,
    )?;
    table.triggers = rows
        .iter()
        .map(|row| Trigger {
            owner: schema.to_string(),
            name: cell(row, 0),
            table_owner: schema.to_string(),
            table_name: name.to_string(),
            description: cell(row, 1),
            trigger_body: cell(row, 2),
        })
        .collect();
    Ok(())
}

fn load_grants(
    session: &Session,
    schema: &str,
    name: &str,
    options: QueryOptions,
    table: &mut Table,
) -> Result<(), String> {
    let rows = fetch_rows(
        session,
        "select grantee, privilege, grantable from all_tab_privs \
         where table_schema = :1 and table_name = :2 order by grantee, privilege",
        &[&schema, &name],
        options,
    )?;
    let mut by_grantee: BTreeMap<(String, bool), Vec<String>> = BTreeMap::new();
    for row in rows {
        by_grantee
            .entry((cell(&row, 0), cell(&row, 2) == "YES"))
            .or_default()
            .push(cell(&row, 1));
    }
    table.grants = by_grantee
        .into_iter()
        .map(|((grantee, grantable), privileges)| Grant {
            grantee,
            privileges,
            grantable,
        })
        .collect();
    Ok(())
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
            comments: None,
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
    load_comments(session, schema, name, options, &mut table)?;
    load_indexes(session, schema, name, options, &mut table)?;
    load_constraints(session, schema, name, options, &mut table)?;
    load_triggers(session, schema, name, options, &mut table)?;
    load_grants(session, schema, name, options, &mut table)?;
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
    Ok(table.write_full(&WriteSettings::default()))
}
