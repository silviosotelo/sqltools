// SPDX-License-Identifier: GPL-3.0-only
//! Basic import: a delimited text file becomes `INSERT` statements in the editor. Like the DDL forms, it
//! never executes anything: the script is reviewed and run by the user.

use serde::Serialize;

use crate::browser::ddl_ident;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CsvPreview {
    /// Column names from the header, or `COL1..COLn` when the file has none.
    pub columns: Vec<String>,
    /// The first rows, trimmed to the preview size.
    pub rows: Vec<Vec<String>>,
    pub total_rows: usize,
}

const DELIMITERS: [char; 2] = [',', ';'];

/// RFC 4180-ish: quoted fields may contain the delimiter, newlines and doubled quotes. The delimiter is
/// detected from the header line (comma or semicolon, whichever is more frequent outside quotes).
pub fn parse_rows(text: &str) -> Result<Vec<Vec<String>>, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let delimiter = detect_delimiter(text);
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if quoted {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                field.push(ch);
            }
            continue;
        }
        match ch {
            '"' if field.is_empty() => quoted = true,
            c if c == delimiter => {
                row.push(std::mem::take(&mut field));
            }
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            other => field.push(other),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    if quoted {
        return Err("The file ends inside a quoted field".to_string());
    }
    // A blank line is not a row, but `,,` is: a record whose fields are all empty still has columns.
    rows.retain(|row| row.len() > 1 || row.iter().any(|field| !field.trim().is_empty()));
    if rows.is_empty() {
        return Err("The file has no rows".to_string());
    }
    let columns = rows[0].len();
    if rows.iter().any(|row| row.len() != columns) {
        return Err(format!(
            "The file is not rectangular: some rows have fewer than {columns} fields"
        ));
    }
    Ok(rows)
}

fn detect_delimiter(text: &str) -> char {
    let first_line = text.lines().next().unwrap_or_default();
    let mut counts = [0usize; 2];
    let mut quoted = false;
    for ch in first_line.chars() {
        if ch == '"' {
            quoted = !quoted;
        } else if !quoted {
            if let Some(index) = DELIMITERS.iter().position(|delimiter| *delimiter == ch) {
                counts[index] += 1;
            }
        }
    }
    if counts[1] > counts[0] {
        DELIMITERS[1]
    } else {
        DELIMITERS[0]
    }
}

pub fn preview(text: &str, has_header: bool, max_rows: usize) -> Result<CsvPreview, String> {
    let rows = parse_rows(text)?;
    let (columns, body) = if has_header {
        (
            rows[0]
                .iter()
                .map(|name| name.trim().to_string())
                .collect::<Vec<_>>(),
            &rows[1..],
        )
    } else {
        (
            (1..=rows[0].len())
                .map(|index| format!("COL{index}"))
                .collect::<Vec<_>>(),
            &rows[..],
        )
    };
    Ok(CsvPreview {
        columns,
        rows: body.iter().take(max_rows).cloned().collect(),
        total_rows: body.len(),
    })
}

fn literal(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "NULL".to_string();
    }
    if trimmed.eq_ignore_ascii_case("null") {
        return "NULL".to_string();
    }
    if trimmed.parse::<f64>().is_ok() && !trimmed.contains(['e', 'E']) {
        return trimmed.to_string();
    }
    format!("'{}'", trimmed.replace('\'', "''"))
}

/// One `INSERT` per row, columns from the header (`COLn` without it). Empty fields and the word NULL
/// become SQL NULL, numbers stay unquoted, everything else is quoted with escaped apostrophes.
pub fn insert_script(
    table: &str,
    text: &str,
    has_header: bool,
    max_rows: usize,
) -> Result<String, String> {
    let table = ddl_ident(table)?;
    let preview = preview(text, has_header, usize::MAX)?;
    if preview.total_rows == 0 {
        return Err("The file has a header but no data rows".to_string());
    }
    let columns = preview
        .columns
        .iter()
        .map(|name| ddl_ident(name))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    let mut script =
        String::from("-- Generated by SQLTools from a delimited file: review before running\n");
    for row in preview.rows.iter().take(max_rows) {
        let values = row
            .iter()
            .map(|value| literal(value))
            .collect::<Vec<_>>()
            .join(", ");
        script.push_str(&format!(
            "insert into {table} ({columns}) values ({values});\n"
        ));
    }
    if preview.total_rows > max_rows {
        script.push_str(&format!(
            "-- {} more rows were not included\n",
            preview.total_rows - max_rows
        ));
    }
    Ok(script)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_fields_keep_delimiters_newlines_and_quotes() {
        let text = "a,b\n\"1,5\",\"line1\nline2\"\n\"say \"\"hi\"\"\",x\n";
        let rows = parse_rows(text).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[1], vec!["1,5".to_string(), "line1\nline2".to_string()]);
        assert_eq!(rows[2][0], r#"say "hi""#);
    }

    #[test]
    fn the_delimiter_is_detected_from_the_header() {
        let rows = parse_rows("a;b;c\n1;2;3\n").unwrap();
        assert_eq!(rows[0].len(), 3);
    }

    #[test]
    fn a_ragged_file_is_refused() {
        assert!(parse_rows("a,b\n1,2,3\n")
            .unwrap_err()
            .contains("rectangular"));
        assert!(parse_rows("").is_err());
    }

    #[test]
    fn the_script_quotes_strings_and_leaves_numbers() {
        let text = "NAME,AGE,NOTE\nO'Brien,42,\n,NULL,x\n";
        let script = insert_script("A11", text, true, 100).unwrap();
        assert!(
            script.contains(
                r#"insert into "A11" ("NAME", "AGE", "NOTE") values ('O''Brien', 42, NULL);"#
            ),
            "{script}"
        );
        assert!(script.contains("values (NULL, NULL, 'x');"), "{script}");
        assert!(!script.contains("'42'"), "{script}");
    }

    #[test]
    fn without_a_header_the_columns_are_positional() {
        let script = insert_script("T", "1,2\n", false, 100).unwrap();
        assert!(
            script.contains(r#"("COL1", "COL2") values (1, 2)"#),
            "{script}"
        );
    }

    #[test]
    fn an_unsafe_table_or_a_header_only_file_is_refused() {
        assert!(insert_script("T\"", "1\n", false, 100).is_err());
        assert!(insert_script("T", "A,B\n", true, 100).is_err());
    }

    #[test]
    fn the_row_cap_is_reported() {
        let text = "N\n1\n2\n3\n";
        let script = insert_script("T", text, true, 2).unwrap();
        assert_eq!(script.matches("insert into").count(), 2, "{script}");
        assert!(script.contains("1 more rows"), "{script}");
    }

    #[test]
    fn an_unterminated_quote_is_refused() {
        assert!(parse_rows("\"a,b\n1,2\n")
            .unwrap_err()
            .contains("quoted field"));
    }

    #[test]
    fn a_row_of_empty_fields_is_kept() {
        let rows = parse_rows("a,b,c\n,,\n1,2,3\n").unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(
            rows[1],
            vec!["".to_string(), "".to_string(), "".to_string()]
        );
        let script = insert_script("T", "a,b,c\n,,\n", true, 10).unwrap();
        assert!(script.contains("values (NULL, NULL, NULL);"), "{script}");
    }
}
