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
    // One execution per session: two scripts sharing the OCI session would
    // interleave result sets and DBMS_OUTPUT lines.
    let _execution = session.lock_execution();
    if session.is_closed() {
        return Err("The connection was closed".to_string());
    }
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
        // `close()` interrupts an active OCI call, but between statements there
        // is nothing to break: re-check so a closed session runs no more work.
        if session.is_closed() {
            return Err("The connection was closed".to_string());
        }
        let text = statement.text.trim();
        if text.is_empty() {
            continue;
        }
        let statement_rows = match run_statement(session, text, options, max_rows, &mut emit) {
            Ok(rows) => rows,
            Err(error) => {
                // Discard the failed statement's output so it cannot leak into
                // the next execution; draining is never fatal. A closed session
                // is not drained: the disconnect would wait for nothing.
                if !session.is_closed() {
                    let _ = session.dbms_output_lines(1000);
                }
                return Err(error);
            }
        };
        total += statement_rows;
        last_rows = statement_rows;
    }
    Ok(ScriptOutcome {
        total_rows: total,
        last_rows,
    })
}

/// Runs one statement, streaming its result set, and emits the
/// `DBMS_OUTPUT` lines it produced.
fn run_statement<F>(
    session: &Session,
    text: &str,
    options: QueryOptions,
    max_rows: usize,
    emit: &mut F,
) -> Result<usize, String>
where
    F: FnMut(ScriptEvent),
{
    if session.is_closed() {
        return Err("The connection was closed".to_string());
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
            // A Batch callback can close the session; do not ask for another
            // batch (or report success) on a closed connection.
            if session.is_closed() {
                return Err("The connection was closed".to_string());
            }
            let batch = handle.next_batch(want).map_err(|e| e.to_string())?;
            if batch.rows.is_empty() {
                break;
            }
            statement_rows += batch.rows.len();
            emit(ScriptEvent::Batch(batch));
        }
    } else {
        if session.is_closed() {
            return Err("The connection was closed".to_string());
        }
        let affected = session.execute(text).map_err(|e| e.to_string())?;
        statement_rows = affected as usize;
    }
    // Draining output is auxiliary: never turn a successful statement into an
    // error because the DBMS_OUTPUT buffer could not be read. Skip it when the
    // session was closed, so the close does not wait on a full drain.
    if !session.is_closed() {
        let lines = session.dbms_output_lines(1000).unwrap_or_default();
        if !lines.is_empty() {
            emit(ScriptEvent::Output(lines));
        }
    }
    Ok(statement_rows)
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
