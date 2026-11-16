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
            let generated = constraint_name.to_ascii_uppercase().starts_with("SYS_C");
            let mut constraint = Constraint {
                name: constraint_name,
                kind,
                generated,
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

/// IOT metadata from the `IOT - TOP` index and its overflow segment, mirroring
/// `Loader_Index::load_iot_overflow`.
fn load_iot_info(
    session: &Session,
    schema: &str,
    name: &str,
    options: QueryOptions,
) -> Result<crate::IotInfo, String> {
    let rows = fetch_rows(
        session,
        "select v1.include_column, v1.pct_threshold, v2.tablespace_name, v2.next_extent \
         from all_indexes v1, all_tables v2 \
         where v1.index_type = 'IOT - TOP' and v1.table_name = v2.iot_name \
         and v1.table_name = :1 and v1.table_owner = v2.owner \
         and v1.table_owner = :2",
        &[&name, &schema],
        options,
    )?;
    let Some(row) = rows.first() else {
        return Ok(crate::IotInfo::default());
    };
    let include_column = match number(row, 0) {
        0 => None,
        value => Some(value as usize),
    };
    Ok(crate::IotInfo {
        pct_threshold: match number(row, 1) {
            0 => None,
            value => Some(value),
        },
        include_column,
        overflow_tablespace: option_cell(row, 2),
        overflow_next_extent: match number(row, 3) {
            0 => None,
            value => Some(value),
        },
    })
}

/// Range/list/hash partitions. Unsupported types return an error so the caller
/// falls back to `DBMS_METADATA`.
fn load_partitioning(
    session: &Session,
    schema: &str,
    name: &str,
    options: QueryOptions,
    row_movement: bool,
    table: &mut Table,
) -> Result<(), String> {
    use crate::{Partition, PartitionKind, Partitioning, Subpartition};

    let rows = fetch_rows(
        session,
        "select partitioning_type, subpartitioning_type, interval \
         from all_part_tables where owner = :1 and table_name = :2",
        &[&schema, &name],
        options,
    )?;
    let Some(row) = rows.first() else {
        return Ok(());
    };
    let sub_kind = match cell(row, 1).as_str() {
        "" | "NONE" => None,
        "RANGE" => Some(PartitionKind::Range),
        "HASH" => Some(PartitionKind::Hash),
        "LIST" => Some(PartitionKind::List),
        other => {
            return Err(format!(
                "{schema}.{name} uses unsupported subpartitioning {other}; falling back to DBMS_METADATA"
            ))
        }
    };
    let kind = match cell(row, 0).as_str() {
        "RANGE" => PartitionKind::Range,
        "HASH" => PartitionKind::Hash,
        "LIST" => PartitionKind::List,
        other => {
            return Err(format!(
            "{schema}.{name} uses unsupported partitioning {other}; falling back to DBMS_METADATA"
        ))
        }
    };
    let interval = {
        let value = cell(row, 2).trim().to_string();
        if value.is_empty() || value == "NO" {
            None
        } else {
            Some(value)
        }
    };
    let columns = fetch_rows(
        session,
        "select column_name from all_part_key_columns \
         where owner = :1 and name = :2 and object_type = 'TABLE' \
         order by column_position",
        &[&schema, &name],
        options,
    )?;
    let partitions = fetch_rows(
        session,
        "select partition_name, high_value, tablespace_name, next_extent \
         from all_tab_partitions where table_owner = :1 and table_name = :2 \
         order by partition_position",
        &[&schema, &name],
        options,
    )?;
    let sub_columns = if sub_kind.is_some() {
        fetch_rows(
            session,
            "select column_name from all_subpart_key_columns \
             where owner = :1 and name = :2 and object_type = 'TABLE' \
             order by column_position",
            &[&schema, &name],
            options,
        )?
        .iter()
        .map(|row| cell(row, 0))
        .collect()
    } else {
        Vec::new()
    };
    let sub_templates = if sub_kind.is_some() {
        fetch_rows(
            session,
            "select subpartition_name, high_bound, tablespace_name \
             from all_subpartition_templates \
             where user_name = :1 and table_name = :2 \
             order by subpartition_position",
            &[&schema, &name],
            options,
        )?
        .iter()
        .map(|row| Subpartition {
            name: cell(row, 0),
            high_value: option_cell(row, 1),
            tablespace: option_cell(row, 2),
        })
        .collect()
    } else {
        Vec::new()
    };
    let mut by_partition: HashMap<String, Vec<Subpartition>> = HashMap::new();
    if sub_kind.is_some() {
        let rows = fetch_rows(
            session,
            "select partition_name, subpartition_name, high_value, tablespace_name \
             from all_tab_subpartitions where table_owner = :1 and table_name = :2 \
             order by partition_name, subpartition_position",
            &[&schema, &name],
            options,
        )?;
        for row in rows {
            by_partition
                .entry(cell(&row, 0))
                .or_default()
                .push(Subpartition {
                    name: cell(&row, 1),
                    high_value: option_cell(&row, 2),
                    tablespace: option_cell(&row, 3),
                });
        }
    }
    table.partitioning = Some(Partitioning {
        kind,
        columns: columns.iter().map(|row| cell(row, 0)).collect(),
        sub_kind,
        sub_columns,
        sub_templates,
        interval,
        partitions: partitions
            .iter()
            .map(|row| {
                let name = cell(row, 0);
                Partition {
                    subpartitions: by_partition.remove(&name).unwrap_or_default(),
                    name,
                    high_value: option_cell(row, 1),
                    tablespace: option_cell(row, 2),
                    next_extent: match number(row, 3) {
                        0 => None,
                        value => Some(value),
                    },
                }
            })
            .collect(),
        row_movement,
    });
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
         nullable, data_default, virtual_column from all_tab_cols \
         where owner = :1 and table_name = :2 and hidden_column = 'NO' \
         order by column_id",
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
            virtual_column: cell(&row, 7) == "YES",
        });
    }
    let storage = fetch_rows(
        session,
        "select tablespace_name, next_extent, temporary, duration, cache, \
         iot_type, row_movement from all_tables \
         where owner = :1 and table_name = :2",
        &[&schema, &name],
        options,
    )?;
    let mut row_movement = false;
    let base = storage
        .iter()
        .find(|row| cell(row, 5) != "IOT_OVERFLOW")
        .or_else(|| storage.first());
    if let Some(row) = base {
        let tablespace = cell(row, 0);
        if !tablespace.is_empty() {
            table.tablespace = Some(tablespace);
        }
        let next = number(row, 1);
        if next > 0 {
            table.next_extent = Some(next);
        }
        if cell(row, 2) == "Y" {
            table.temporary = Some(match cell(row, 3).as_str() {
                "SYS$TRANSACTION" => crate::TemporaryDuration::Delete,
                _ => crate::TemporaryDuration::Preserve,
            });
        }
        table.cache = cell(row, 4) == "Y";
        row_movement = cell(row, 6) == "ENABLED";
        if cell(row, 5) == "IOT" {
            table.iot = Some(load_iot_info(session, schema, name, options)?);
        }
    }
    load_partitioning(session, schema, name, options, row_movement, &mut table)?;
    if table.iot.is_some() && table.partitioning.is_some() {
        return Err(format!(
            "{schema}.{name} is a partitioned IOT; falling back to DBMS_METADATA"
        ));
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
