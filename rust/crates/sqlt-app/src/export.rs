// SPDX-License-Identifier: GPL-3.0-only
//! Streaming CSV export from the Oracle driver (no full result in JS memory).

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use sqlt_core::{ColumnMeta, Row, Value};
use sqlt_oracle::{QueryOptions, Session};

pub fn csv_escape(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

pub fn value_csv(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Binary(bytes) => bytes.iter().map(|byte| format!("{byte:02X}")).collect(),
        other => csv_escape(other.display().unwrap_or_default()),
    }
}

pub fn write_csv<W: Write>(
    writer: &mut W,
    columns: &[ColumnMeta],
    rows: &[Row],
) -> std::io::Result<()> {
    let header: Vec<String> = columns
        .iter()
        .map(|column| csv_escape(&column.name))
        .collect();
    writeln!(writer, "{}", header.join(","))?;
    for row in rows {
        let line: Vec<String> = row.iter().map(value_csv).collect();
        writeln!(writer, "{}", line.join(","))?;
    }
    Ok(())
}

/// Runs the query again and writes every row to `path`. `max_rows == 0` means
/// no limit. Returns the number of rows written.
pub fn export_csv(
    session: &Session,
    sql: &str,
    options: QueryOptions,
    path: &Path,
    max_rows: usize,
) -> Result<usize, String> {
    let mut query = session.query(sql, options).map_err(|e| e.to_string())?;
    let columns = query.columns().to_vec();
    let file = File::create(path).map_err(|e| e.to_string())?;
    let mut writer = BufWriter::new(file);
    let header: Vec<String> = columns
        .iter()
        .map(|column| csv_escape(&column.name))
        .collect();
    writeln!(writer, "{}", header.join(",")).map_err(|e| e.to_string())?;
    let mut total = 0usize;
    loop {
        let want = if max_rows == 0 {
            1000
        } else {
            (max_rows - total).min(1000)
        };
        if want == 0 {
            break;
        }
        let batch = query.next_batch(want).map_err(|e| e.to_string())?;
        if batch.rows.is_empty() {
            break;
        }
        for row in &batch.rows {
            let line: Vec<String> = row.iter().map(value_csv).collect();
            writeln!(writer, "{}", line.join(",")).map_err(|e| e.to_string())?;
        }
        total += batch.rows.len();
    }
    writer.flush().map_err(|e| e.to_string())?;
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_escapes_and_nulls() {
        assert_eq!(csv_escape("plain"), "plain");
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("a\"b"), "\"a\"\"b\"");
        assert_eq!(value_csv(&Value::Null), "");
        assert_eq!(value_csv(&Value::Text("a,b".to_string())), "\"a,b\"");
        assert_eq!(value_csv(&Value::Number("42".to_string())), "42");
        assert_eq!(value_csv(&Value::Binary(vec![0xDE, 0xAD])), "DEAD");
    }

    #[test]
    fn writes_header_and_rows() {
        let columns = vec![
            ColumnMeta {
                name: "N".to_string(),
                type_name: "NUMBER".to_string(),
                nullable: false,
                numeric: true,
            },
            ColumnMeta {
                name: "T".to_string(),
                type_name: "VARCHAR2".to_string(),
                nullable: true,
                numeric: false,
            },
        ];
        let rows = vec![vec![
            Value::Number("1".to_string()),
            Value::Text("a,b".to_string()),
        ]];
        let mut output = Vec::new();
        write_csv(&mut output, &columns, &rows).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "N,T\n1,\"a,b\"\n");
    }
}
