// SPDX-License-Identifier: GPL-3.0-only
//! Script execution: split the buffer with `sqlt_core::split` and run each
//! statement, streaming result sets as they come.

use sqlt_core::{split, statement_at};
use sqlt_core::{Batch, ColumnMeta};
use sqlt_oracle::{QueryOptions, Session};

#[derive(Debug)]
pub enum ScriptEvent {
    Columns(Vec<ColumnMeta>),
    Batch(Batch),
    /// `DBMS_OUTPUT` lines drained after a statement.
    Output(Vec<String>),
}

/// Statements that produce a result set. Everything else (DML, DDL, PL/SQL)
/// goes through `Session::execute`.
fn returns_rows(sql: &str) -> bool {
    let first = sql
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(first.as_str(), "SELECT" | "WITH")
}

/// Result of a script run. `last_rows` is what the grid shows; `total_rows`
/// sums every statement. Stops on the first error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptOutcome {
    pub total_rows: usize,
    pub last_rows: usize,
}

/// Runs the whole script, or only the statement at `offset` (UTF-16 position).
pub fn run_script<F>(
    session: &Session,
    sql: &str,
    options: QueryOptions,
    max_rows: usize,
    offset: Option<usize>,
    mut emit: F,
) -> Result<ScriptOutcome, String>
where
    F: FnMut(ScriptEvent),
{
    let mut statements = split(sql);
    if let Some(position) = offset {
        let index = statement_at(&statements, position)
            .ok_or_else(|| "There is no statement under the cursor".to_string())?;
        statements = vec![statements.swap_remove(index)];
    }
    if statements.is_empty() {
        return Err("The script does not contain an executable statement".to_string());
    }

    let mut total = 0usize;
    let mut last_rows = 0usize;
    for statement in statements {
        let text = statement.text.trim();
        if text.is_empty() {
            continue;
        }
        let mut statement_rows = 0usize;
        if returns_rows(text) {
            let mut handle = session.query(text, options).map_err(|e| e.to_string())?;
            emit(ScriptEvent::Columns(handle.columns().to_vec()));
            loop {
                let want = if max_rows == 0 {
                    500
                } else {
                    max_rows.saturating_sub(statement_rows).min(500)
                };
                if want == 0 {
                    break;
                }
                let batch = handle.next_batch(want).map_err(|e| e.to_string())?;
                if batch.rows.is_empty() {
                    break;
                }
                statement_rows += batch.rows.len();
                emit(ScriptEvent::Batch(batch));
            }
        } else {
            let affected = session.execute(text).map_err(|e| e.to_string())?;
            statement_rows = affected as usize;
        }
        // Draining output is auxiliary: never turn a successful statement into
        // an error because the DBMS_OUTPUT buffer could not be read.
        let lines = session.dbms_output_lines(1000).unwrap_or_default();
        if !lines.is_empty() {
            emit(ScriptEvent::Output(lines));
        }
        total += statement_rows;
        last_rows = statement_rows;
    }
    Ok(ScriptOutcome {
        total_rows: total,
        last_rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_select_and_with_return_rows() {
        assert!(returns_rows("select 1 from dual"));
        assert!(returns_rows(
            "  WITH x AS (select 1 from dual) select * from x"
        ));
        assert!(!returns_rows("update t set a = 1"));
        assert!(!returns_rows("begin null; end;"));
    }
}
