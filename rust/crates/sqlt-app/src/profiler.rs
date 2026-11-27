// SPDX-License-Identifier: GPL-3.0-only
//! PL/SQL Profiler: runs one statement under `DBMS_PROFILER` and returns the time spent per source line.
//! The run number comes back through an OUT bind, which `execute_with` cannot read, so the block is run
//! with `Session::execute_block`.

use serde::Serialize;
use sqlt_oracle::{OracleType, QueryOptions, Session};

use crate::catalog::{cell, fetch_rows};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileLine {
    pub unit: String,
    pub unit_type: String,
    pub line: u32,
    /// Nanoseconds spent on that line.
    pub total_time: u64,
    pub total_occur: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileResult {
    pub run_id: i64,
    /// True when the profiler sequence had to be created to make the run possible.
    pub prepared: bool,
    /// Lines with time, most expensive first.
    pub lines: Vec<ProfileLine>,
    /// Sum of the reported lines, in nanoseconds.
    pub total_time: u64,
}

/// `DBMS_PROFILER` needs its own sequence in the schema; `profload.sql` creates it, but a schema can
/// have the tables and not the sequence (that is exactly what `ORA-06528` means). It is idempotent.
fn ensure_ready(session: &Session, options: QueryOptions) -> Result<bool, String> {
    let rows = fetch_rows(
        session,
        "select count(*) from user_sequences where sequence_name = 'PLSQL_PROFILER_RUNNUMBER'",
        &[],
        options,
    )?;
    let present = rows
        .first()
        .map(|row| cell(row, 0))
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    if present > 0 {
        return Ok(false);
    }
    session
        .execute_with(
            "create sequence plsql_profiler_runnumber minvalue 1 \
             maxvalue 999999999999999999999999999 increment by 1 start with 1 nocache",
            &[],
        )
        .map_err(|error| {
            format!("DBMS_PROFILER is not ready and the sequence could not be created: {error}")
        })?;
    Ok(true)
}

/// Starts the profiler, runs `sql`, stops it and reads the per-line data of that run.
pub fn profile(
    session: &Session,
    sql: &str,
    limit: u32,
    options: QueryOptions,
) -> Result<ProfileResult, String> {
    let prepared = ensure_ready(session, options)?;
    let started = session.execute_block(
        "begin dbms_profiler.start_profiler(run_number => :1); end;",
        &[&OracleType::Number(0, 0)],
        &[1],
    )?;
    let run_id: i64 = started
        .first()
        .and_then(|value| value.as_deref())
        .and_then(|value| value.trim().parse().ok())
        .ok_or_else(|| "DBMS_PROFILER did not return a run number".to_string())?;

    let executed = session.execute_with(sql, &[]);
    // Always stop the profiler: leaving it open would profile everything that runs afterwards.
    let stopped = session.execute_block("begin dbms_profiler.stop_profiler; end;", &[], &[]);
    executed.map_err(|error| error.to_string())?;
    stopped?;

    let lines = read_lines(session, run_id, limit, options)?;
    let total_time = lines.iter().map(|line| line.total_time).sum();
    Ok(ProfileResult {
        run_id,
        prepared,
        lines,
        total_time,
    })
}

/// Most expensive lines of one run, with the source text when it is visible to the user.
pub fn read_lines(
    session: &Session,
    run_id: i64,
    limit: u32,
    options: QueryOptions,
) -> Result<Vec<ProfileLine>, String> {
    let rows = fetch_rows(
        session,
        "select u.unit_name, u.unit_type, d.line#, d.total_time, d.total_occur, s.text \
         from plsql_profiler_data d \
         join plsql_profiler_units u on u.runid = d.runid and u.unit_number = d.unit_number \
         left join user_source s on s.name = u.unit_name and s.type = u.unit_type and s.line = d.line# \
         where d.runid = :1 and d.total_time > 0 \
         order by d.total_time desc, u.unit_name, d.line# \n         fetch first :2 rows only",
        &[&run_id, &limit],
        options,
    )?;
    Ok(rows
        .iter()
        .map(|row| ProfileLine {
            unit: cell(row, 0),
            unit_type: cell(row, 1),
            line: cell(row, 2).parse().unwrap_or(0),
            total_time: cell(row, 3).parse().unwrap_or(0),
            total_occur: cell(row, 4).parse().unwrap_or(0),
            text: cell(row, 5),
        })
        .collect())
}

#[cfg(test)]
mod tests {

    /// The UI formats nanoseconds; keep the rule in one place and test it.
    pub fn format_duration(nanos: u64) -> String {
        if nanos >= 1_000_000_000 {
            format!("{:.3} s", nanos as f64 / 1e9)
        } else if nanos >= 1_000_000 {
            format!("{:.3} ms", nanos as f64 / 1e6)
        } else if nanos >= 1_000 {
            format!("{:.3} us", nanos as f64 / 1e3)
        } else {
            format!("{nanos} ns")
        }
    }

    #[test]
    fn durations_are_readable() {
        assert_eq!(format_duration(999), "999 ns");
        assert_eq!(format_duration(1_500), "1.500 us");
        assert_eq!(format_duration(2_500_000), "2.500 ms");
        assert_eq!(format_duration(1_250_000_000), "1.250 s");
    }
}
