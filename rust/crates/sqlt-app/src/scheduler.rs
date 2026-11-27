// SPDX-License-Identifier: GPL-3.0-only
//! Read-only view of `DBMS_SCHEDULER` jobs.

use serde::Serialize;
use sqlt_oracle::{QueryOptions, Session, ToSql};

use crate::catalog::{cell, fetch_rows};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerJob {
    pub name: String,
    pub owner: String,
    pub enabled: bool,
    pub state: String,
    pub repeat_interval: String,
    pub next_run: String,
}

pub fn list_jobs(
    session: &Session,
    schema: Option<&str>,
    options: QueryOptions,
) -> Result<Vec<SchedulerJob>, String> {
    let sql = if schema.is_some() {
        "select job_name, owner, enabled, state, repeat_interval, \
         to_char(next_run_date, 'YYYY-MM-DD HH24:MI:SS') \
         from all_scheduler_jobs where owner = :1 order by job_name"
    } else {
        // `user_scheduler_jobs` has no OWNER column: the pseudo-column gives it.
        "select job_name, user, enabled, state, repeat_interval, \
         to_char(next_run_date, 'YYYY-MM-DD HH24:MI:SS') \
         from user_scheduler_jobs order by job_name"
    };
    let owner = schema.map(str::to_string);
    let bindings: Vec<&dyn ToSql> = owner.iter().map(|value| value as &dyn ToSql).collect();
    let rows = fetch_rows(session, sql, &bindings, options)?;
    Ok(rows
        .iter()
        .map(|row| SchedulerJob {
            name: cell(row, 0),
            owner: cell(row, 1),
            enabled: cell(row, 2) == "TRUE",
            state: cell(row, 3),
            repeat_interval: cell(row, 4),
            next_run: cell(row, 5),
        })
        .collect())
}
