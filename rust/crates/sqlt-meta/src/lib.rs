// SPDX-License-Identifier: GPL-3.0-only
//! First slice of the SQLTools MetaDictionary port: table and trigger DDL in
//! the same layout as the Windows C++ implementation.
//!
//! Golden source: `tests/meta_test.cpp`. The full Loader/MetaObjects port is
//! incremental; this covers CREATE TABLE columns/storage and trigger headers.

pub mod loader;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteSettings {
    pub generate_prompts: bool,
    pub storage_clause: bool,
    pub lower_db_name: bool,
}

impl Default for WriteSettings {
    fn default() -> Self {
        WriteSettings {
            generate_prompts: false,
            storage_clause: true,
            lower_db_name: true,
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
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    pub owner: String,
    pub name: String,
    pub columns: Vec<TabColumn>,
    pub tablespace: Option<String>,
    pub next_extent: Option<i64>,
}

impl Table {
    pub fn new(owner: &str, name: &str) -> Self {
        Table {
            owner: owner.to_string(),
            name: name.to_string(),
            columns: Vec::new(),
            tablespace: None,
            next_extent: None,
        }
    }

    fn display_name(name: &str, lower: bool) -> String {
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

    fn column_null_text(column: &TabColumn) -> String {
        if !column.nullable {
            "NOT NULL".to_string()
        } else if !column.data_default.is_empty() {
            format!("DEFAULT {} NULL", column.data_default)
        } else {
            "NULL".to_string()
        }
    }

    pub fn write_definition(&self, settings: &WriteSettings) -> String {
        let lower = settings.lower_db_name;
        let table_name = Table::display_name(&self.name, lower);
        if self.columns.is_empty() {
            return format!("CREATE TABLE {table_name} (\n)\n/\n\n");
        }
        let max_name = self
            .columns
            .iter()
            .map(|column| Table::raw_name_len(&column.name))
            .max()
            .unwrap_or(0);
        let types: Vec<String> = self.columns.iter().map(Table::column_type).collect();
        let max_type = types.iter().map(String::len).max().unwrap_or(0);
        let mut out = format!("CREATE TABLE {table_name} (\n");
        for (index, column) in self.columns.iter().enumerate() {
            let display = Table::display_name(&column.name, lower);
            let raw_len = Table::raw_name_len(&column.name);
            let name_pad = " ".repeat(max_name + 1 - raw_len);
            let type_pad = " ".repeat(max_type.saturating_sub(types[index].len()) + 1);
            let comma = if index + 1 < self.columns.len() {
                ","
            } else {
                ""
            };
            out.push_str(&format!(
                "  {display}{name_pad}{}{type_pad}{}{comma}\n",
                types[index],
                Table::column_null_text(column)
            ));
        }

        let storage =
            settings.storage_clause && (self.tablespace.is_some() || self.next_extent.is_some());
        if storage {
            out.push_str(")\n");
            if let Some(tablespace) = &self.tablespace {
                out.push_str(&format!(
                    "  TABLESPACE {}\n",
                    Table::display_name(tablespace, lower)
                ));
            }
            if let Some(next) = self.next_extent {
                let value = if next % 1024 == 0 {
                    format!("{} K", next / 1024)
                } else {
                    format!("{next}")
                };
                out.push_str("  STORAGE (\n");
                out.push_str(&format!("    NEXT       {value}\n"));
                out.push_str("  )\n");
            }
        } else {
            out.push_str(")\n");
        }
        out.push_str("/\n\n");
        out
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
        out.push('\n');
        out
    }
}
