// SPDX-License-Identifier: GPL-3.0-only
//! Oracle cell values, without precision loss.

use serde::{Deserialize, Serialize};

/// Typed cell value. NUMBER travels as a canonical string (38 digits).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
pub enum Value {
    Null,
    Text(String),
    Number(String),
    Date(String),
    Timestamp(String),
    Binary(Vec<u8>),
}

impl Value {
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// Display text. `None` for NULL (the UI decides how to render it) and for
    /// binary values (the UI renders them as hexadecimal when needed).
    pub fn display(&self) -> Option<&str> {
        match self {
            Value::Null | Value::Binary(_) => None,
            Value::Text(s) | Value::Number(s) | Value::Date(s) | Value::Timestamp(s) => Some(s),
        }
    }
}

/// Result column metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMeta {
    pub name: String,
    pub type_name: String,
    pub nullable: bool,
    pub numeric: bool,
}

/// One result row.
pub type Row = Vec<Value>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Batch {
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Row>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_and_binary_have_no_text() {
        assert!(Value::Null.is_null());
        assert_eq!(Value::Null.display(), None);
        assert_eq!(Value::Binary(vec![1, 2]).display(), None);
    }

    #[test]
    fn numbers_preserve_the_string() {
        let v = Value::Number("12345678901234567890123456789012345678".to_string());
        assert_eq!(v.display(), Some("12345678901234567890123456789012345678"));
    }
}
