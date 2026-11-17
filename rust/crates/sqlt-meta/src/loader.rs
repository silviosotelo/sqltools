// SPDX-License-Identifier: GPL-3.0-only
//! Oracle loader for the MetaDictionary table slice: columns, storage,
//! indexes, constraints, triggers, comments and grants.

use std::collections::{BTreeMap, HashMap};

use sqlt_core::Value;
use sqlt_oracle::{QueryOptions, Session};

use crate::{Constraint, Grant, Index, IndexStorage, TabColumn, Table, Trigger, WriteSettings};

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

struct StorageDefaults {
    default_tablespace: Option<String>,
    next_extents: HashMap<String, i64>,
}

/// `Loader::Init` + `TableStorage::BuildDefault`: the connected user's default
/// tablespace and each tablespace's default `NEXT`, used to suppress values the
/// original considers defaults (`MetaObjects.cpp:487-519`).
fn load_storage_defaults(
    session: &Session,
    options: QueryOptions,
) -> Result<StorageDefaults, String> {
    let mut defaults = StorageDefaults {
        default_tablespace: None,
        next_extents: HashMap::new(),
    };
    if let Some(row) = fetch_rows(
        session,
        "select default_tablespace from user_users",
        &[],
        options,
    )?
    .first()
    {
        defaults.default_tablespace = option_cell(row, 0);
    }
    for row in fetch_rows(
        session,
        "select tablespace_name, next_extent from user_tablespaces",
        &[],
        options,
    )? {
        let next = number(&row, 1);
        if next > 0 {
            defaults.next_extents.insert(cell(&row, 0), next);
        }
    }
    Ok(defaults)
}

impl StorageDefaults {
    fn is_default_tablespace(&self, tablespace: &str) -> bool {
        self.default_tablespace
            .as_deref()
            .map(|default| default.eq_ignore_ascii_case(tablespace))
            .unwrap_or(false)
    }

    fn default_next_extent(&self, tablespace: Option<&str>) -> Option<i64> {
        tablespace.and_then(|name| self.next_extents.get(name).copied())
    }

    /// Drops the tablespace when it is the user's default and the `NEXT` when it
    /// equals the tablespace default; `None` `tablespace` keeps the value.
    fn storage(
        &self,
        tablespace: Option<String>,
        next_extent: Option<i64>,
    ) -> (Option<String>, Option<i64>) {
        let tablespace = tablespace.filter(|name| !self.is_default_tablespace(name));
        let next_extent = match (next_extent, tablespace.as_deref()) {
            (Some(next), Some(name)) if self.default_next_extent(Some(name)) == Some(next) => None,
            (next, _) => next,
        };
        (tablespace, next_extent)
    }
}

/// `generate_unique_name` (`TableTransformer.cpp:179-211`): `<table[..26]>_$NN`
/// for the first name that `ALL_OBJECTS` does not have.
pub fn unique_backup_name(
    session: &Session,
    owner: &str,
    table: &str,
    options: QueryOptions,
) -> Result<String, String> {
    let base: String = table.chars().take(26).collect();
    for index in 0..100 {
        let candidate = format!("{base}_${index:02}");
        let rows = fetch_rows(
            session,
            "select 1 from all_objects where owner = :1 and object_name = :2",
            &[&owner, &candidate],
            options,
        )?;
        if rows.is_empty() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "Cannot generate a unique name for {owner}.{table} (100 candidates are taken)"
    ))
}

/// `(referencing owner, table, constraint)` → `(delete rule, columns, ref columns)`.
type DependentKey = (String, String, String);
type DependentColumns = (String, Vec<String>, Vec<String>);

/// Foreign keys from other tables that point at `owner.table`
/// (`TableTransformer.cpp:431-451`).
pub fn dependent_foreign_keys(
    session: &Session,
    owner: &str,
    table: &str,
    options: QueryOptions,
) -> Result<Vec<(Table, Constraint)>, String> {
    let rows = fetch_rows(
        session,
        "select c.owner, c.table_name, c.constraint_name, cc.column_name, \
         p.table_name, rc.column_name, c.delete_rule \
         from all_constraints c \
         join all_constraints p on p.owner = c.r_owner and p.constraint_name = c.r_constraint_name \
         join all_cons_columns cc on cc.owner = c.owner and cc.constraint_name = c.constraint_name \
         join all_cons_columns rc on rc.owner = p.owner and rc.constraint_name = p.constraint_name \
           and rc.position = cc.position \
         where c.constraint_type = 'R' and p.owner = :1 and p.table_name = :2 \
           and p.constraint_type in ('P', 'U') \
         order by c.owner, c.table_name, c.constraint_name, cc.position",
        &[&owner, &table],
        options,
    )?;
    let mut by_key: BTreeMap<DependentKey, DependentColumns> = BTreeMap::new();
    for row in rows {
        let key = (cell(&row, 0), cell(&row, 1), cell(&row, 2));
        let entry = by_key
            .entry(key)
            .or_insert_with(|| (cell(&row, 6), Vec::new(), Vec::new()));
        entry.1.push(cell(&row, 3));
        entry.2.push(cell(&row, 5));
    }
    Ok(by_key
        .into_iter()
        .map(
            |((ref_owner, ref_table, name), (delete_rule, columns, ref_columns))| {
                (
                    Table::new(&ref_owner, &ref_table),
                    Constraint {
                        name,
                        kind: 'R',
                        generated: false,
                        columns,
                        condition: None,
                        ref_table: Some(table.to_string()),
                        ref_owner: Some(owner.to_string()),
                        ref_columns,
                        delete_rule: if delete_rule.is_empty() {
                            None
                        } else {
                            Some(delete_rule)
                        },
                        no_validate: false,
                        deferrable: false,
                        deferred: false,
                        rely: false,
                        disabled: false,
                        using_index: None,
                    },
                )
            },
        )
        .collect())
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
    defaults: &StorageDefaults,
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
         c.column_name, i.next_extent, i.compression, i.prefix_length from all_indexes i \
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
        let (tablespace, next_extent) = defaults.storage(
            option_cell(&row, 3),
            match number(&row, 5) {
                0 => None,
                value => Some(value),
            },
        );
        let entry = by_name.entry(index_name.clone()).or_insert_with(|| Index {
            name: index_name,
            unique: cell(&row, 1) == "UNIQUE",
            bitmap: cell(&row, 2).contains("BITMAP"),
            columns: Vec::new(),
            tablespace,
            next_extent,
            compress: (cell(&row, 6) == "ENABLED").then(|| number(&row, 7) as usize),
        });
        let column = cell(&row, 4);
        if !column.is_empty() {
            entry.columns.push(column);
        }
    }
    // `ALL_IND_EXPRESSIONS`: a function-based column appears as a hidden
    // `SYS_NC…` name in `ALL_IND_COLUMNS`, so the expression replaces it.
    let expression_rows = fetch_rows(
        session,
        "select e.index_name, e.column_position, e.column_expression \
         from all_ind_expressions e \
         join all_indexes i on i.owner = e.index_owner and i.index_name = e.index_name \
         where i.owner = :1 and i.table_name = :2",
        &[&schema, &name],
        options,
    )?;
    for row in expression_rows {
        let index_name = cell(&row, 0);
        let position = number(&row, 1);
        let expression = cell(&row, 2);
        if expression.is_empty() || position < 1 {
            continue;
        }
        if let Some(index) = by_name.get_mut(&index_name) {
            if let Some(column) = index.columns.get_mut(position as usize - 1) {
                *column = expression;
            }
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
    defaults: &StorageDefaults,
    table: &mut Table,
) -> Result<(), String> {
    let referenced_rows = fetch_rows(
        session,
        "select r.owner, r.constraint_name, r.table_name, rc.column_name \
         from all_constraints c \
         join all_constraints r on r.owner = nvl(c.r_owner, c.owner) \
           and r.constraint_name = c.r_constraint_name \
         join all_cons_columns rc on rc.owner = r.owner and rc.constraint_name = r.constraint_name \
         where c.owner = :1 and c.table_name = :2 and c.constraint_type = 'R' \
         order by r.owner, r.constraint_name, rc.position",
        &[&schema, &name],
        options,
    )?;
    let mut referenced: HashMap<(String, String), (String, Vec<String>)> = HashMap::new();
    for row in referenced_rows {
        let entry = referenced
            .entry((cell(&row, 0), cell(&row, 1)))
            .or_insert_with(|| (cell(&row, 2), Vec::new()));
        entry.1.push(cell(&row, 3));
    }

    let rows = fetch_rows(
        session,
        "select c.constraint_name, c.constraint_type, c.search_condition, \
         c.r_constraint_name, c.delete_rule, cc.column_name, c.generated, c.validated, \
         c.deferrable, c.deferred, c.rely, c.status, c.r_owner, \
         i.tablespace_name, i.next_extent \
         from all_constraints c \
         left join all_cons_columns cc on cc.owner = c.owner and cc.constraint_name = c.constraint_name \
         left join all_indexes i on i.owner = c.owner and i.index_name = c.index_name \
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
            // ALL_CONSTRAINTS.GENERATED: 'GENERATED NAME' or 'USER NAME'.
            let generated = cell(&row, 6) == "GENERATED NAME";
            let (tablespace, next_extent) = defaults.storage(
                option_cell(&row, 13),
                match number(&row, 14) {
                    0 => None,
                    value => Some(value),
                },
            );
            let using_index = if tablespace.is_some() || next_extent.is_some() {
                Some(IndexStorage {
                    tablespace,
                    next_extent,
                })
            } else {
                None
            };
            let mut constraint = Constraint {
                name: constraint_name,
                kind,
                generated,
                columns: Vec::new(),
                condition: option_cell(&row, 2),
                ref_table: None,
                ref_owner: None,
                ref_columns: Vec::new(),
                delete_rule: option_cell(&row, 4),
                no_validate: cell(&row, 7) == "NOT VALIDATED",
                deferrable: cell(&row, 8) == "DEFERRABLE",
                deferred: cell(&row, 9) == "DEFERRED",
                rely: cell(&row, 10) == "RELY",
                disabled: cell(&row, 11) == "DISABLED",
                using_index,
            };
            if kind == 'R' {
                let ref_owner = option_cell(&row, 12);
                let owner = ref_owner.clone().unwrap_or_else(|| schema.to_string());
                if let Some((ref_table, ref_columns)) = referenced.get(&(owner, cell(&row, 3))) {
                    constraint.ref_table = Some(ref_table.clone());
                    constraint.ref_columns = ref_columns.clone();
                    constraint.ref_owner = ref_owner;
                }
            }
            constraint
        });
        let column = cell(&row, 5);
        if !column.is_empty() {
            entry.columns.push(column);
        }
    }
    for constraint in by_name.values() {
        if constraint.kind == 'R' && constraint.ref_table.is_none() {
            return Err(format!(
                "{schema}.{name}: foreign key {} references a constraint that ALL_CONSTRAINTS does not expose",
                constraint.name
            ));
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
    // `Loader_Trigger.cpp` selects every trigger of the table (it reads
    // `status` but never filters on it and `Trigger::Write` ignores it).
    // `Loader_Trigger.cpp` looks triggers up by `table_owner`, so a trigger
    // living in another schema still shows up on this table.
    let rows = fetch_rows(
        session,
        "select owner, trigger_name, description, trigger_body, when_clause, action_type \
         from all_triggers \
         where table_owner = :1 and table_name = :2 \
         order by trigger_name",
        &[&schema, &name],
        options,
    )?;
    table.triggers = rows
        .iter()
        .map(|row| Trigger {
            owner: cell(row, 0),
            name: cell(row, 1),
            table_owner: schema.to_string(),
            table_name: name.to_string(),
            description: cell(row, 2),
            trigger_body: cell(row, 3),
            when_clause: option_cell(row, 4),
            action_type: cell(row, 5),
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
         where table_schema = :1 and table_name = :2",
        &[&schema, &name],
        options,
    )?;
    // Column-level grants live in `ALL_COL_PRIVS` (`Loader_Grant.cpp`), not in
    // `ALL_TAB_PRIVS`, and the original writes them as separate statements.
    let column_rows = fetch_rows(
        session,
        "select grantee, privilege, column_name, grantable from all_col_privs \
         where table_schema = :1 and table_name = :2",
        &[&schema, &name],
        options,
    )?;
    let mut by_grantee: BTreeMap<(String, bool), Grant> = BTreeMap::new();
    let entry = |grantee: String, grantable: bool| Grant {
        grantee,
        privileges: Vec::new(),
        grantable,
        column_privileges: Vec::new(),
    };
    for row in rows {
        by_grantee
            .entry((cell(&row, 0), cell(&row, 2) == "YES"))
            .or_insert_with_key(|(grantee, grantable)| entry(grantee.clone(), *grantable))
            .privileges
            .push(cell(&row, 1));
    }
    for row in column_rows {
        let grant = by_grantee
            .entry((cell(&row, 0), cell(&row, 3) == "YES"))
            .or_insert_with_key(|(grantee, grantable)| entry(grantee.clone(), *grantable));
        let privilege = cell(&row, 1);
        let column = cell(&row, 2);
        match grant
            .column_privileges
            .iter_mut()
            .find(|(name, _)| name == &privilege)
        {
            Some((_, columns)) => columns.push(column),
            None => grant.column_privileges.push((privilege, vec![column])),
        }
    }
    for grant in by_grantee.values_mut() {
        grant.privileges.sort();
        grant.column_privileges.sort_by(|a, b| a.0.cmp(&b.0));
        for (_, columns) in &mut grant.column_privileges {
            columns.sort();
        }
    }
    table.grants = by_grantee.into_values().collect();
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
    // The index exists even when the IOT has no overflow segment, so it is
    // queried without joining ALL_TABLES (the overflow is optional).
    let index_rows = fetch_rows(
        session,
        "select index_name, include_column, pct_threshold from all_indexes \
         where index_type = 'IOT - TOP' and table_owner = :1 and table_name = :2",
        &[&schema, &name],
        options,
    )?;
    let Some(index) = index_rows.first() else {
        return Ok(crate::IotInfo::default());
    };
    let index_name = cell(index, 0);
    let include_column = match number(index, 1) {
        0 => None,
        value => Some(value as usize),
    };
    let overflow_rows = fetch_rows(
        session,
        "select v2.tablespace_name, v2.next_extent \
         from all_indexes v1, all_tables v2 \
         where v1.index_type = 'IOT - TOP' and v1.table_name = v2.iot_name \
         and v1.table_name = :1 and v1.table_owner = v2.owner \
         and v1.table_owner = :2",
        &[&name, &schema],
        options,
    )?;
    let (overflow_tablespace, overflow_next_extent) = match overflow_rows.first() {
        Some(row) => (
            option_cell(row, 0),
            match number(row, 1) {
                0 => None,
                value => Some(value),
            },
        ),
        None => (None, None),
    };
    let partitioning = load_iot_partitions(session, schema, &index_name, options)?;
    if include_column.is_some() && partitioning.is_some() {
        return Err(format!(
            "{schema}.{name} is a partitioned IOT with overflow; falling back to DBMS_METADATA"
        ));
    }
    Ok(crate::IotInfo {
        pct_threshold: match number(index, 2) {
            0 => None,
            value => Some(value),
        },
        include_column,
        overflow_tablespace,
        overflow_next_extent,
        partitioning,
    })
}

/// Index partitions of the `IOT - TOP` index (`Index::writeIndexPartitions`).
fn load_iot_partitions(
    session: &Session,
    schema: &str,
    index_name: &str,
    options: QueryOptions,
) -> Result<Option<crate::IotPartitioning>, String> {
    use crate::{IotPartition, IotPartitioning, PartitionKind};

    if index_name.is_empty() {
        return Ok(None);
    }
    let rows = fetch_rows(
        session,
        "select partitioning_type, subpartitioning_type from all_part_indexes \
         where owner = :1 and index_name = :2",
        &[&schema, &index_name],
        options,
    )?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    let subpartitioning = cell(row, 1);
    if !subpartitioning.is_empty() && subpartitioning != "NONE" {
        return Err(format!(
            "{schema}.{index_name} is a subpartitioned IOT index; falling back to DBMS_METADATA"
        ));
    }
    let kind = match cell(row, 0).as_str() {
        "RANGE" => PartitionKind::Range,
        "HASH" => PartitionKind::Hash,
        "LIST" => PartitionKind::List,
        other => {
            return Err(format!(
                "{schema}.{index_name} uses unsupported IOT partitioning {other}; falling back to DBMS_METADATA"
            ))
        }
    };
    let columns = fetch_rows(
        session,
        "select column_name from all_part_key_columns \
         where owner = :1 and name = :2 and object_type = 'INDEX' \
         order by column_position",
        &[&schema, &index_name],
        options,
    )?;
    let partitions = fetch_rows(
        session,
        "select partition_name, high_value, tablespace_name, next_extent \
         from all_ind_partitions where index_owner = :1 and index_name = :2 \
         order by partition_position",
        &[&schema, &index_name],
        options,
    )?;
    Ok(Some(IotPartitioning {
        kind,
        columns: columns.iter().map(|row| cell(row, 0)).collect(),
        partitions: partitions
            .iter()
            .map(|row| IotPartition {
                name: cell(row, 0),
                high_value: option_cell(row, 1),
                tablespace: option_cell(row, 2),
                next_extent: match number(row, 3) {
                    0 => None,
                    value => Some(value),
                },
            })
            .collect(),
    }))
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
    let defaults = load_storage_defaults(session, options)?;
    let mut row_movement = false;
    let base = storage
        .iter()
        .find(|row| cell(row, 5) != "IOT_OVERFLOW")
        .or_else(|| storage.first());
    if let Some(row) = base {
        let (tablespace, next) = defaults.storage(
            option_cell(row, 0),
            match number(row, 1) {
                0 => None,
                value => Some(value),
            },
        );
        table.tablespace = tablespace;
        table.next_extent = next;
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
    if table.iot.is_none() {
        load_partitioning(session, schema, name, options, row_movement, &mut table)?;
    }
    load_comments(session, schema, name, options, &mut table)?;
    load_indexes(session, schema, name, options, &defaults, &mut table)?;
    load_constraints(session, schema, name, options, &defaults, &mut table)?;
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
    load_table_ddl_with_settings(session, schema, name, options, &WriteSettings::default())
}

pub fn load_table_ddl_with_settings(
    session: &Session,
    schema: Option<&str>,
    name: &str,
    options: QueryOptions,
    settings: &WriteSettings,
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
    Ok(table.write_full(settings))
}
