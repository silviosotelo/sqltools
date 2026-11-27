// SPDX-License-Identifier: GPL-3.0-only
//! Cell editing inside the result grid. Only single-table queries qualify (decided in
//! `web/src/grid/editable.ts`); this module performs the UPDATE and reports a conflict when the row
//! changed since it was read.

use sqlt_oracle::{QueryOptions, Session};

use crate::browser::ddl_ident;

/// The UPDATE always matches the row by ROWID and re-checks the old value, so a row changed by somebody
/// else is not overwritten silently. `NULL` is compared as `NULL`, not with `=`.
pub fn update_statement(table: &str, column: &str) -> Result<String, String> {
    let table = qualified(table)?;
    let column = ddl_ident(column)?;
    Ok(format!(
        "update {table} set {column} = :1 where rowid = :2 and ({column} = :3 or ({column} is null and :3 is null))"
    ))
}

/// `owner.table` or `table` as the user wrote it. Kept unquoted when it is a plain identifier so Oracle
/// folds it to upper case (`a11` must find `A11`); quoted only when it needs it. Quoted names are refused
/// because they could break out of the quotes.
fn qualified(table: &str) -> Result<String, String> {
    let parts: Vec<&str> = table.split('.').collect();
    let render = |name: &str| -> Result<String, String> {
        if name.contains('"') || name.contains('\0') || name.trim().is_empty() {
            return Err(format!("Unsafe table name: {table}"));
        }
        let plain = name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '$' | '#'));
        Ok(if plain {
            name.to_string()
        } else {
            format!("\"{name}\"")
        })
    };
    match parts.as_slice() {
        [name] => render(name),
        [owner, name] => Ok(format!("{}.{}", render(owner)?, render(name)?)),
        _ => Err(format!("Unsafe table name: {table}")),
    }
}

/// Whether `<table>` accepts a ROWID column. Views built on `GROUP BY`, `DISTINCT` and friends reject it
/// with ORA-01446, and the grid must stay read-only for them *before* rewriting the query.
pub fn rowid_available(
    session: &Session,
    table: &str,
    options: QueryOptions,
) -> Result<bool, String> {
    let sql = format!("select rowid from {} where rownum = 0", qualified(table)?);
    match session.query(&sql, options) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Applies one cell change. `Ok(true)` = exactly one row updated; `Ok(false)` = the row no longer
/// matches what was read (conflict); `Err` = the database refused it.
pub fn update_cell(
    session: &Session,
    table: &str,
    column: &str,
    rowid: &str,
    old_value: Option<&str>,
    new_value: Option<&str>,
    _options: QueryOptions,
) -> Result<bool, String> {
    if rowid.trim().is_empty() {
        return Err("The row has no ROWID".to_string());
    }
    let sql = update_statement(table, column)?;
    let changed = session
        .execute_with(&sql, &[&new_value, &rowid, &old_value])
        .map_err(|error| error.to_string())?;
    Ok(changed == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statement_matches_by_rowid_and_old_value() {
        assert_eq!(
            update_statement("A11", "TIPO_CTO").unwrap(),
            "update A11 set \"TIPO_CTO\" = :1 where rowid = :2 and (\"TIPO_CTO\" = :3 or (\"TIPO_CTO\" is null and :3 is null))"
        );
        assert_eq!(
            update_statement("ANAMNESIS.A11", "N").unwrap(),
            "update ANAMNESIS.A11 set \"N\" = :1 where rowid = :2 and (\"N\" = :3 or (\"N\" is null and :3 is null))"
        );
    }

    #[test]
    fn unsafe_names_are_refused() {
        assert!(update_statement("A11\"", "N").is_err());
        assert!(update_statement("A.B.C", "N").is_err());
        assert!(update_statement("A11", "N\" or 1=1 --").is_err());
    }
}
