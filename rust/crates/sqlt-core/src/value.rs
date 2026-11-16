// SPDX-License-Identifier: GPL-3.0-only
//! Valores de una celda Oracle, sin pérdida de precisión.

/// Valor tipado de una celda. NUMBER viaja como string canónica (38 dígitos).
#[derive(Debug, Clone, PartialEq, Eq)]
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

    /// Texto listo para mostrar. `None` para NULL (la UI decide cómo pintarlo)
    /// y para binarios (la UI los muestra en hexadecimal si hace falta).
    pub fn display(&self) -> Option<&str> {
        match self {
            Value::Null | Value::Binary(_) => None,
            Value::Text(s) | Value::Number(s) | Value::Date(s) | Value::Timestamp(s) => Some(s),
        }
    }
}

/// Metadata de una columna del resultado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnMeta {
    pub name: String,
    pub type_name: String,
    pub nullable: bool,
    pub numeric: bool,
}

/// Filas de un lote de resultados.
pub type Row = Vec<Value>;

#[derive(Debug, Clone, PartialEq, Eq)]
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
