// SPDX-License-Identifier: GPL-3.0-only
//! SQLTools MetaDictionary port (incremental): table, index, constraint,
//! trigger, comment and grant DDL with the Windows C++ layout.
//!
//! Golden source: `tests/meta_test.cpp` plus the formatting rules in
//! `src/MetaDictionary/MetaObjects.cpp`.

pub mod compare;
pub mod loader;

pub use compare::{compare_ddl, DdlComparison};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteSettings {
    pub generate_prompts: bool,
    pub storage_clause: bool,
    pub lower_db_name: bool,
    pub comments: bool,
    pub grants: bool,
    pub indexes: bool,
    pub constraints: bool,
    pub triggers: bool,
}

impl Default for WriteSettings {
    fn default() -> Self {
        WriteSettings {
            generate_prompts: false,
            storage_clause: true,
            lower_db_name: true,
            comments: true,
            grants: true,
            indexes: true,
            constraints: true,
            triggers: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabColumn {
    pub name: String,
    pub data_type: String,
    pub data_length: i64,
    pub precision: i64,
    pub scale: i64,
    pub nullable: bool,
    pub data_default: String,
    pub comments: Option<String>,
    /// `ALL_TAB_COLUMNS.virtual_column = 'YES'`; `data_default` holds the
    /// expression, rendered as ` AS (expr)` by `MetaObjects.cpp`.
    pub virtual_column: bool,
}

impl TabColumn {
    pub fn new(name: &str, data_type: &str) -> Self {
        TabColumn {
            name: name.to_string(),
            data_type: data_type.to_string(),
            data_length: 0,
            precision: 0,
            scale: 0,
            nullable: true,
            data_default: String::new(),
            comments: None,
            virtual_column: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionKind {
    Range,
    Hash,
    List,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subpartition {
    pub name: String,
    pub high_value: Option<String>,
    pub tablespace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partition {
    pub name: String,
    /// `ALL_TAB_PARTITIONS.high_value` (LONG), printed verbatim.
    pub high_value: Option<String>,
    pub tablespace: Option<String>,
    pub next_extent: Option<i64>,
    pub subpartitions: Vec<Subpartition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partitioning {
    pub kind: PartitionKind,
    pub columns: Vec<String>,
    pub sub_kind: Option<PartitionKind>,
    pub sub_columns: Vec<String>,
    pub sub_templates: Vec<Subpartition>,
    pub interval: Option<String>,
    pub partitions: Vec<Partition>,
    pub row_movement: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IotPartition {
    pub name: String,
    pub high_value: Option<String>,
    pub tablespace: Option<String>,
    pub next_extent: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IotPartitioning {
    pub kind: PartitionKind,
    pub columns: Vec<String>,
    pub partitions: Vec<IotPartition>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IotInfo {
    /// `ALL_INDEXES.pct_threshold` of the `IOT - TOP` index.
    pub pct_threshold: Option<i64>,
    /// `ALL_INDEXES.include_column` (1-based column position).
    pub include_column: Option<usize>,
    pub overflow_tablespace: Option<String>,
    pub overflow_next_extent: Option<i64>,
    /// Index partitions (`Index::writeIndexPartitions` for `IOT - TOP`).
    pub partitioning: Option<IotPartitioning>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemporaryDuration {
    /// `ON COMMIT PRESERVE ROWS`
    Preserve,
    /// `ON COMMIT DELETE ROWS`
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Index {
    pub name: String,
    pub unique: bool,
    pub bitmap: bool,
    pub columns: Vec<String>,
    pub tablespace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraint {
    pub name: String,
    /// Oracle constraint type: C, P, U or R.
    pub kind: char,
    /// `SYS_C...` names are system-generated and are not printed.
    pub generated: bool,
    pub columns: Vec<String>,
    pub condition: Option<String>,
    pub ref_table: Option<String>,
    pub ref_columns: Vec<String>,
    pub delete_rule: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grant {
    pub grantee: String,
    pub privileges: Vec<String>,
    pub grantable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub owner: String,
    pub name: String,
    pub columns: Vec<TabColumn>,
    pub tablespace: Option<String>,
    pub next_extent: Option<i64>,
    /// `None` for a permanent table; `ALL_TABLES.duration` otherwise.
    pub temporary: Option<TemporaryDuration>,
    /// `ALL_TABLES.cache = 'Y'` adds a `CACHE` line after the storage clause.
    pub cache: bool,
    /// Range/list/hash partitioning (`Table::writePartitions`).
    pub partitioning: Option<Partitioning>,
    /// `Some` for index-organized tables (`ALL_TABLES.iot_type = 'IOT'`).
    pub iot: Option<IotInfo>,
    pub comments: Option<String>,
    pub indexes: Vec<Index>,
    pub constraints: Vec<Constraint>,
    pub triggers: Vec<Trigger>,
    pub grants: Vec<Grant>,
}

fn write_storage_clause(
    out: &mut String,
    indent: &str,
    tablespace: Option<&str>,
    next: Option<i64>,
    lower: bool,
) {
    if let Some(tablespace) = tablespace {
        out.push_str(&format!(
            "{indent}TABLESPACE {}\n",
            Table::display_name(tablespace, lower)
        ));
    }
    if let Some(next) = next {
        let value = if next % 1024 == 0 {
            format!("{} K", next / 1024)
        } else {
            format!("{next}")
        };
        out.push_str(&format!(
            "{indent}STORAGE (\n{indent}  NEXT       {value}\n{indent})\n"
        ));
    }
}

/// `Index::writeIndexPartitions` for an `IOT - TOP` index: no GLOBAL/LOCAL,
/// key columns at +2 and per-partition storage at +4.
fn write_iot_partitions(
    out: &mut String,
    partitioning: &IotPartitioning,
    table_tablespace: Option<&str>,
    settings: &WriteSettings,
) {
    let lower = settings.lower_db_name;
    let keyword = match partitioning.kind {
        PartitionKind::Range => "RANGE",
        PartitionKind::Hash => "HASH",
        PartitionKind::List => "LIST",
    };
    if partitioning.kind == PartitionKind::List {
        let column = partitioning.columns.first().cloned().unwrap_or_default();
        out.push_str(&format!(
            "  PARTITION BY LIST ({})\n",
            Table::display_name(&column, lower)
        ));
    } else {
        out.push_str(&format!("  PARTITION BY {keyword} (\n"));
        for (index, column) in partitioning.columns.iter().enumerate() {
            let comma = if index + 1 < partitioning.columns.len() {
                ","
            } else {
                ""
            };
            out.push_str(&format!(
                "    {}{comma}\n",
                Table::display_name(column, lower)
            ));
        }
        out.push_str("  )\n");
    }
    out.push_str("  (\n");
    let max_name = partitioning
        .partitions
        .iter()
        .map(|partition| partition.name.len())
        .max()
        .unwrap_or(0);
    let count = partitioning.partitions.len();
    for (index, partition) in partitioning.partitions.iter().enumerate() {
        let mut block = format!(
            "    PARTITION {}",
            Table::display_name(&partition.name, lower)
        );
        if partitioning.kind != PartitionKind::Hash {
            let pad = max_name + 1 - partition.name.len();
            let values = if partitioning.kind == PartitionKind::Range {
                "VALUES LESS THAN"
            } else {
                "VALUES"
            };
            block.push_str(&" ".repeat(pad));
            block.push_str(&format!(
                "{values} ({})",
                partition.high_value.clone().unwrap_or_default()
            ));
        }
        if settings.storage_clause {
            if let Some(tablespace) = &partition.tablespace {
                let same =
                    table_tablespace.is_some_and(|table| table.eq_ignore_ascii_case(tablespace));
                if !same {
                    block.push_str(&format!(
                        "\n      TABLESPACE {}",
                        Table::display_name(tablespace, lower)
                    ));
                }
            }
            if let Some(next) = partition.next_extent {
                let value = if next % 1024 == 0 {
                    format!("{} K", next / 1024)
                } else {
                    format!("{next}")
                };
                block.push_str(&format!(
                    "\n      STORAGE (\n        NEXT       {value}\n      )"
                ));
            }
        }
        if index + 1 < count {
            block.push(',');
        }
        out.push_str(&block);
        out.push('\n');
    }
    out.push_str("  )\n");
}

/// One line per subpartition, names padded like `writeSubpartitions`.
fn subpartition_lines(
    items: &[Subpartition],
    kind: PartitionKind,
    indent: &str,
    table_tablespace: Option<&str>,
    lower: bool,
) -> String {
    let max_name = items.iter().map(|item| item.name.len()).max().unwrap_or(0);
    let mut out = String::new();
    for (index, item) in items.iter().enumerate() {
        out.push_str(&format!(
            "{indent}SUBPARTITION {}",
            Table::display_name(&item.name, lower)
        ));
        if kind != PartitionKind::Hash {
            let pad = max_name + 1 - item.name.len();
            let values = if kind == PartitionKind::Range {
                "VALUES LESS THAN"
            } else {
                "VALUES"
            };
            out.push_str(&" ".repeat(pad));
            out.push_str(&format!(
                "{values} ({})",
                item.high_value.clone().unwrap_or_default()
            ));
        }
        if let Some(tablespace) = &item.tablespace {
            let same = table_tablespace.is_some_and(|table| table.eq_ignore_ascii_case(tablespace));
            if !same {
                out.push_str(&format!(
                    " TABLESPACE {}",
                    Table::display_name(tablespace, lower)
                ));
            }
        }
        if index + 1 < items.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out
}

/// `Table::writeSubpartitions`: per-partition subpartitions are omitted when
/// they match the template (the template covers them).
fn subpartitions_are_default(
    partition: &Partition,
    templates: &[Subpartition],
    table_tablespace: Option<&str>,
) -> bool {
    if templates.is_empty() || partition.subpartitions.len() != templates.len() {
        return false;
    }
    partition
        .subpartitions
        .iter()
        .zip(templates)
        .all(|(actual, template)| {
            if actual.high_value != template.high_value {
                return false;
            }
            match &template.tablespace {
                Some(tablespace) => actual
                    .tablespace
                    .as_deref()
                    .is_some_and(|value| value.eq_ignore_ascii_case(tablespace)),
                None => match (table_tablespace, actual.tablespace.as_deref()) {
                    (Some(table), Some(value)) => table.eq_ignore_ascii_case(value),
                    (None, None) => true,
                    _ => false,
                },
            }
        })
}

impl Table {
    pub fn new(owner: &str, name: &str) -> Self {
        Table {
            owner: owner.to_string(),
            name: name.to_string(),
            columns: Vec::new(),
            tablespace: None,
            next_extent: None,
            temporary: None,
            cache: false,
            partitioning: None,
            iot: None,
            comments: None,
            indexes: Vec::new(),
            constraints: Vec::new(),
            triggers: Vec::new(),
            grants: Vec::new(),
        }
    }

    pub(crate) fn display_name(name: &str, lower: bool) -> String {
        if name.starts_with('"') && name.ends_with('"') {
            name.to_string()
        } else if lower {
            name.to_lowercase()
        } else {
            name.to_string()
        }
    }

    fn raw_name_len(name: &str) -> usize {
        if name.starts_with('"') && name.ends_with('"') && name.len() >= 2 {
            name.len() - 2
        } else {
            name.len()
        }
    }

    fn column_type(column: &TabColumn) -> String {
        let upper = column.data_type.to_ascii_uppercase();
        if upper.contains('(') {
            return column.data_type.clone();
        }
        match upper.as_str() {
            "NUMBER" => {
                if column.precision > 0 {
                    format!("NUMBER({},{})", column.precision, column.scale)
                } else {
                    "NUMBER".to_string()
                }
            }
            "VARCHAR2" | "NVARCHAR2" | "CHAR" | "NCHAR" | "RAW" if column.data_length > 0 => {
                format!("{}({})", column.data_type, column.data_length)
            }
            _ => column.data_type.clone(),
        }
    }

    /// `MetaObjects::Table::writeColumns`: virtual columns use ` AS (expr)`,
    /// defaults use ` DEFAULT expr`, and the nullability is always appended.
    fn column_qualifier(column: &TabColumn) -> String {
        let mut out = String::new();
        if column.virtual_column {
            out.push_str(&format!("AS ({})", column.data_default));
        } else if !column.data_default.is_empty() {
            out.push_str(&format!("DEFAULT {}", column.data_default));
        }
        if !out.is_empty() {
            out.push(' ');
        }
        if column.nullable {
            out.push_str("NULL");
        } else {
            out.push_str("NOT NULL");
        }
        out
    }

    pub fn write_definition(&self, settings: &WriteSettings) -> String {
        let lower = settings.lower_db_name;
        let table_name = Table::display_name(&self.name, lower);
        let temporary = if self.temporary.is_some() {
            "GLOBAL TEMPORARY "
        } else {
            ""
        };
        if self.columns.is_empty() {
            return format!("CREATE {temporary}TABLE {table_name} (\n)\n/\n\n");
        }
        let max_name = self
            .columns
            .iter()
            .map(|column| Table::raw_name_len(&column.name))
            .max()
            .unwrap_or(0);
        let types: Vec<String> = self.columns.iter().map(Table::column_type).collect();
        let max_type = types.iter().map(String::len).max().unwrap_or(0);
        let mut out = format!("CREATE {temporary}TABLE {table_name} (\n");
        for (index, column) in self.columns.iter().enumerate() {
            let display = Table::display_name(&column.name, lower);
            let raw_len = Table::raw_name_len(&column.name);
            let name_pad = " ".repeat(max_name + 1 - raw_len);
            let type_pad = " ".repeat(max_type.saturating_sub(types[index].len()) + 1);
            let comma = if index + 1 < self.columns.len() || self.iot.is_some() {
                ","
            } else {
                ""
            };
            out.push_str(&format!(
                "  {display}{name_pad}{}{type_pad}{}{comma}\n",
                types[index],
                Table::column_qualifier(column)
            ));
        }

        // `Table::WritePrimaryKeyForIOT`: the PK is part of the column list.
        if self.iot.is_some() {
            if let Some(pk) = self
                .constraints
                .iter()
                .find(|constraint| constraint.kind == 'P')
            {
                let name = if pk.generated {
                    String::new()
                } else {
                    format!("CONSTRAINT {} ", Table::display_name(&pk.name, lower))
                };
                out.push_str(&format!("  {name}PRIMARY KEY (\n"));
                for (index, column) in pk.columns.iter().enumerate() {
                    let comma = if index + 1 < pk.columns.len() {
                        ","
                    } else {
                        ""
                    };
                    out.push_str(&format!(
                        "    {}{comma}\n",
                        Table::display_name(column, lower)
                    ));
                }
                out.push_str("  )\n");
            }
        }

        out.push_str(")\n");
        // `MetaObjects::Table::WriteDefinition`: a temporary table replaces the
        // storage clause with ON COMMIT, then CACHE is printed either way.
        if let Some(iot) = &self.iot {
            out.push_str("  ORGANIZATION INDEX");
            if let Some(position) = iot.include_column {
                if let Some(column) = position
                    .checked_sub(1)
                    .and_then(|index| self.columns.get(index))
                {
                    out.push_str(&format!(
                        " INCLUDING {}",
                        Table::display_name(&column.name, lower)
                    ));
                }
            }
            out.push('\n');
            if settings.storage_clause {
                write_storage_clause(
                    &mut out,
                    "    ",
                    self.tablespace.as_deref(),
                    self.next_extent,
                    lower,
                );
            }
            if iot.include_column.is_some() {
                if let Some(threshold) = iot.pct_threshold.filter(|value| *value != 50) {
                    out.push_str(&format!("  PCTTHRESHOLD {threshold:>3}\n"));
                }
                out.push_str("  OVERFLOW\n");
                if settings.storage_clause {
                    write_storage_clause(
                        &mut out,
                        "    ",
                        iot.overflow_tablespace.as_deref(),
                        iot.overflow_next_extent,
                        lower,
                    );
                }
            }
            if let Some(partitioning) = &iot.partitioning {
                write_iot_partitions(&mut out, partitioning, self.tablespace.as_deref(), settings);
            }
        } else if let Some(duration) = self.temporary {
            out.push_str(match duration {
                TemporaryDuration::Preserve => "  ON COMMIT PRESERVE ROWS\n",
                TemporaryDuration::Delete => "  ON COMMIT DELETE ROWS\n",
            });
        } else if settings.storage_clause {
            write_storage_clause(
                &mut out,
                "  ",
                self.tablespace.as_deref(),
                self.next_extent,
                lower,
            );
        }
        if let Some(partitioning) = &self.partitioning {
            self.write_partitions(&mut out, partitioning, settings);
        }
        if self.cache {
            out.push_str("  CACHE\n");
        }
        out.push_str("/\n\n");
        out
    }

    /// Port of `MetaObjects::Table::writePartitions` for range/list/hash.
    /// `ponytail:` a partition tablespace is printed when it differs from the
    /// table's tablespace; the original compares it with the owner's default
    /// segment, which needs a dictionary lookup we do not have here.
    fn write_partitions(
        &self,
        out: &mut String,
        partitioning: &Partitioning,
        settings: &WriteSettings,
    ) {
        let lower = settings.lower_db_name;
        let keyword = match partitioning.kind {
            PartitionKind::Range => "RANGE",
            PartitionKind::Hash => "HASH",
            PartitionKind::List => "LIST",
        };
        if partitioning.kind == PartitionKind::List {
            let column = partitioning.columns.first().cloned().unwrap_or_default();
            out.push_str(&format!(
                "  PARTITION BY LIST ({})\n",
                Table::display_name(&column, lower)
            ));
        } else {
            out.push_str(&format!("  PARTITION BY {keyword} (\n"));
            for (index, column) in partitioning.columns.iter().enumerate() {
                let comma = if index + 1 < partitioning.columns.len() {
                    ","
                } else {
                    ""
                };
                out.push_str(&format!(
                    "    {}{comma}\n",
                    Table::display_name(column, lower)
                ));
            }
            out.push_str("  )\n");
        }
        if let Some(interval) = &partitioning.interval {
            out.push_str(&format!("  INTERVAL ({interval})\n"));
        }
        if let Some(sub_kind) = partitioning.sub_kind {
            let sub_keyword = match sub_kind {
                PartitionKind::Range => "RANGE",
                PartitionKind::Hash => "HASH",
                PartitionKind::List => "LIST",
            };
            if sub_kind == PartitionKind::List {
                let column = partitioning
                    .sub_columns
                    .first()
                    .cloned()
                    .unwrap_or_default();
                out.push_str(&format!(
                    "  SUBPARTITION BY LIST ({})\n",
                    Table::display_name(&column, lower)
                ));
            } else {
                out.push_str(&format!("  SUBPARTITION BY {sub_keyword} (\n"));
                for (index, column) in partitioning.sub_columns.iter().enumerate() {
                    let comma = if index + 1 < partitioning.sub_columns.len() {
                        ","
                    } else {
                        ""
                    };
                    out.push_str(&format!(
                        "    {}{comma}\n",
                        Table::display_name(column, lower)
                    ));
                }
                out.push_str("  )\n");
            }
        }
        if !partitioning.sub_templates.is_empty() {
            out.push_str("  SUBPARTITION TEMPLATE\n  (\n");
            out.push_str(&subpartition_lines(
                &partitioning.sub_templates,
                partitioning.sub_kind.unwrap_or(PartitionKind::Range),
                "    ",
                self.tablespace.as_deref(),
                lower,
            ));
            out.push_str("  )\n");
        }
        out.push_str("  (\n");
        let max_name = partitioning
            .partitions
            .iter()
            .map(|partition| partition.name.len())
            .max()
            .unwrap_or(0);
        let count = partitioning.partitions.len();
        for (index, partition) in partitioning.partitions.iter().enumerate() {
            let mut block = format!(
                "    PARTITION {}",
                Table::display_name(&partition.name, lower)
            );
            if partitioning.kind != PartitionKind::Hash {
                let pad = max_name + 1 - partition.name.len();
                let values = if partitioning.kind == PartitionKind::Range {
                    "VALUES LESS THAN"
                } else {
                    "VALUES"
                };
                block.push_str(&" ".repeat(pad));
                block.push_str(&format!(
                    "{values} ({})",
                    partition.high_value.clone().unwrap_or_default()
                ));
            }
            if let Some(tablespace) = &partition.tablespace {
                let same = self
                    .tablespace
                    .as_deref()
                    .is_some_and(|table| table.eq_ignore_ascii_case(tablespace));
                if !same {
                    block.push_str(&format!(
                        "\n      TABLESPACE {}",
                        Table::display_name(tablespace, lower)
                    ));
                }
            }
            if let Some(next) = partition.next_extent {
                let value = if next % 1024 == 0 {
                    format!("{} K", next / 1024)
                } else {
                    format!("{next}")
                };
                block.push_str(&format!(
                    "\n      STORAGE (\n        NEXT       {value}\n      )"
                ));
            }
            if !partition.subpartitions.is_empty()
                && !subpartitions_are_default(
                    partition,
                    &partitioning.sub_templates,
                    self.tablespace.as_deref(),
                )
            {
                block.push_str("\n      (\n");
                block.push_str(&subpartition_lines(
                    &partition.subpartitions,
                    partitioning.sub_kind.unwrap_or(PartitionKind::Range),
                    "        ",
                    self.tablespace.as_deref(),
                    lower,
                ));
                block.push_str("      )");
            }
            if index + 1 < count {
                block.push(',');
            }
            out.push_str(&block);
            out.push('\n');
        }
        out.push_str("  )\n");
        if partitioning.row_movement {
            out.push_str("  ENABLE ROW MOVEMENT\n");
        }
    }

    fn write_comments(&self, settings: &WriteSettings) -> String {
        let table = Table::display_name(&self.name, settings.lower_db_name);
        let lower = settings.lower_db_name;
        let mut out = String::new();
        if let Some(comment) = &self.comments {
            out.push_str(&format!(
                "COMMENT ON TABLE {table} IS {};\n\n",
                quote_sql(comment)
            ));
        }
        for column in &self.columns {
            if let Some(comment) = &column.comments {
                out.push_str(&format!(
                    "COMMENT ON COLUMN {table}.{} IS {};\n",
                    Table::display_name(&column.name, lower),
                    quote_sql(comment)
                ));
            }
        }
        if self.columns.iter().any(|column| column.comments.is_some()) {
            out.push('\n');
        }
        out
    }

    fn write_grants(&self, settings: &WriteSettings) -> String {
        let table = Table::display_name(&self.name, settings.lower_db_name);
        let mut out = String::new();
        for grant in &self.grants {
            let mut privileges = grant.privileges.clone();
            privileges.sort();
            out.push_str(&format!(
                "GRANT {} ON {table} TO {}{};\n",
                privileges.join(", "),
                Table::display_name(&grant.grantee, settings.lower_db_name),
                if grant.grantable {
                    " WITH GRANT OPTION"
                } else {
                    ""
                }
            ));
        }
        out
    }

    pub fn write_full(&self, settings: &WriteSettings) -> String {
        let mut out = String::new();
        out.push_str(&self.write_definition(settings));
        if settings.indexes {
            for index in &self.indexes {
                out.push_str(&index.write(self, settings));
            }
        }
        if settings.constraints {
            for kind in ['C', 'P', 'U', 'R'] {
                for constraint in self.constraints.iter().filter(|c| c.kind == kind) {
                    out.push_str(&constraint.write(self, settings));
                }
            }
        }
        if settings.triggers {
            for trigger in &self.triggers {
                out.push_str(&trigger.write(settings));
            }
        }
        if settings.comments {
            out.push_str(&self.write_comments(settings));
        }
        if settings.grants {
            out.push_str(&self.write_grants(settings));
        }
        out
    }
}

fn quote_sql(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

impl Index {
    pub fn write(&self, table: &Table, settings: &WriteSettings) -> String {
        let lower = settings.lower_db_name;
        let kind = if self.bitmap {
            "BITMAP "
        } else if self.unique {
            "UNIQUE "
        } else {
            ""
        };
        let mut out = format!(
            "CREATE {kind}INDEX {}\n  ON {} (\n",
            Table::display_name(&self.name, lower),
            Table::display_name(&table.name, lower)
        );
        for (index, column) in self.columns.iter().enumerate() {
            out.push_str(&format!(
                "    {}{}\n",
                Table::display_name(column, lower),
                if index + 1 < self.columns.len() {
                    ","
                } else {
                    ""
                }
            ));
        }
        out.push_str("  )\n");
        if let Some(tablespace) = &self.tablespace {
            out.push_str(&format!(
                "  TABLESPACE {}\n",
                Table::display_name(tablespace, lower)
            ));
        }
        out.push_str("/\n\n");
        out
    }
}

impl Constraint {
    fn kind_text(&self) -> &'static str {
        match self.kind {
            'P' => "PRIMARY KEY",
            'U' => "UNIQUE",
            'R' => "FOREIGN KEY",
            _ => "CHECK",
        }
    }

    pub fn write(&self, table: &Table, settings: &WriteSettings) -> String {
        let lower = settings.lower_db_name;
        let mut out = format!(
            "ALTER TABLE {}\n  ADD CONSTRAINT {} {} (\n",
            Table::display_name(&table.name, lower),
            Table::display_name(&self.name, lower),
            self.kind_text()
        );
        if self.kind == 'C' {
            if let Some(condition) = &self.condition {
                out.push_str(&format!("    {condition}\n"));
            }
        } else {
            for (index, column) in self.columns.iter().enumerate() {
                out.push_str(&format!(
                    "    {}{}\n",
                    Table::display_name(column, lower),
                    if index + 1 < self.columns.len() {
                        ","
                    } else {
                        ""
                    }
                ));
            }
        }
        if self.kind == 'R' {
            if let Some(ref_table) = &self.ref_table {
                out.push_str(&format!(
                    "  ) REFERENCES {} (\n",
                    Table::display_name(ref_table, lower)
                ));
                for (index, column) in self.ref_columns.iter().enumerate() {
                    out.push_str(&format!(
                        "    {}{}\n",
                        Table::display_name(column, lower),
                        if index + 1 < self.ref_columns.len() {
                            ","
                        } else {
                            ""
                        }
                    ));
                }
                out.push_str("  )");
                if let Some(rule) = &self.delete_rule {
                    if rule == "CASCADE" || rule == "SET NULL" {
                        out.push_str(&format!(" ON DELETE {rule}"));
                    }
                }
                out.push('\n');
            }
        } else {
            out.push_str("  )\n");
        }
        out.push_str("/\n\n");
        out
    }
}

impl Grant {
    pub fn write(&self, table: &Table, settings: &WriteSettings) -> String {
        let lower = settings.lower_db_name;
        let mut privileges = self.privileges.clone();
        privileges.sort();
        format!(
            "GRANT {} ON {} TO {}{};\n",
            privileges.join(", "),
            Table::display_name(&table.name, lower),
            Table::display_name(&self.grantee, lower),
            if self.grantable {
                " WITH GRANT OPTION"
            } else {
                ""
            }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    pub owner: String,
    pub name: String,
    pub table_owner: String,
    pub table_name: String,
    pub description: String,
    pub trigger_body: String,
}

impl Trigger {
    fn replace_ci(text: &str, from: &str, to: &str) -> String {
        if from.is_empty() {
            return text.to_string();
        }
        let lower = text.to_ascii_lowercase();
        let needle = from.to_ascii_lowercase();
        let mut out = String::new();
        let mut start = 0usize;
        while let Some(found) = lower[start..].find(&needle) {
            let abs = start + found;
            out.push_str(&text[start..abs]);
            out.push_str(to);
            start = abs + needle.len();
        }
        out.push_str(&text[start..]);
        out
    }

    pub fn write(&self, settings: &WriteSettings) -> String {
        let lower = settings.lower_db_name;
        let name = Table::display_name(&self.name, lower);
        let table = Table::display_name(&self.table_name, lower);
        let mut out = format!("CREATE OR REPLACE TRIGGER {name}\n");
        for line in self.description.lines().skip(1) {
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                continue;
            }
            out.push_str(&Trigger::replace_ci(trimmed, &self.table_name, &table));
            out.push('\n');
        }
        out.push_str(&self.trigger_body);
        out.push_str("\n/\n\n");
        out
    }
}
