// SPDX-License-Identifier: GPL-3.0-only
//! Oracle session: connection, batched queries, binds, LOBs, cancellation and
//! read-only mode.

use std::io::Read;
use std::sync::{Arc, OnceLock};

use oracle::sql_type::{Clob, IntervalDS, IntervalYM, Nclob, OracleType, Timestamp};
use oracle::{Connection, Connector, Privilege};
use sqlt_core::datefmt::{compile_date_format, DateFormat};
use sqlt_core::{Batch, ColumnMeta, Value};

use crate::oci::{locate_oci_dir, prepare_oci_environment, OciError};
use crate::spec::{ConnectMode, ConnectSpec, ConnectTarget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryOptions {
    pub prefetch: u32,
    /// Returns LOB handles and reads them on the client (required for large CLOB/BLOB).
    pub lob_locator: bool,
    /// Maximum bytes read per LOB.
    pub lob_limit: usize,
}

impl Default for QueryOptions {
    fn default() -> Self {
        QueryOptions {
            prefetch: 1000,
            lob_locator: true,
            lob_limit: 4 * 1024 * 1024,
        }
    }
}

pub struct Session {
    conn: Arc<Connection>,
    read_only: bool,
    /// Use the local fast formatter only when the session language is English.
    fast_dates: bool,
    dbms_output_enabled: std::sync::atomic::AtomicBool,
    /// Serializes executions on this OCI session: result sets and
    /// `DBMS_OUTPUT` belong to the session, so two concurrent scripts would
    /// otherwise drain each other's lines.
    execution: std::sync::Mutex<()>,
    /// Set when the connection is being closed or replaced. A script that
    /// starts after that must not run on the orphaned session.
    closed: std::sync::atomic::AtomicBool,
}

pub struct QueryHandle {
    rs: oracle::ResultSet<'static, oracle::Row>,
    conn: Arc<Connection>,
    columns: Vec<ColumnMeta>,
    fast_dates: bool,
    lob_locator: bool,
    lob_limit: usize,
}

impl Session {
    pub fn open(spec: &ConnectSpec) -> Result<Session, OciError> {
        let lib_dir = match &spec.oci_dir {
            Some(dir) => dir.clone(),
            None => locate_oci_dir()?,
        };
        let config_dir = resolve_config_dir(spec)?;
        prepare_oci_environment(&lib_dir, config_dir.as_deref())?;

        let mut connector = Connector::new(
            spec.user.as_str(),
            spec.password.as_str(),
            spec.connect_string(),
        );
        match spec.mode {
            ConnectMode::Normal => {}
            ConnectMode::SysDba => {
                connector.privilege(Privilege::Sysdba);
            }
            ConnectMode::SysOper => {
                connector.privilege(Privilege::Sysoper);
            }
        }
        let conn = connector.connect()?;
        let fast_dates = session_uses_english_dates(&conn);
        Ok(Session {
            conn: Arc::new(conn),
            read_only: spec.read_only,
            fast_dates,
            dbms_output_enabled: std::sync::atomic::AtomicBool::new(false),
            execution: std::sync::Mutex::new(()),
            closed: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Marks the session as closed and interrupts a running statement. Callers
    /// that hold the execution guard should wait for it before dropping the
    /// connection (see `AppState::disconnect`).
    pub fn close(&self) {
        self.closed
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let _ = self.cancel();
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Blocks until this session is free. Callers hold the guard for the whole
    /// statement cycle (execution plus `DBMS_OUTPUT` drain).
    pub fn lock_execution(&self) -> std::sync::MutexGuard<'_, ()> {
        self.execution
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Non-blocking variant, used to detect a running execution.
    pub fn try_lock_execution(&self) -> Option<std::sync::MutexGuard<'_, ()>> {
        self.execution.try_lock().ok()
    }

    pub fn fast_dates(&self) -> bool {
        self.fast_dates
    }

    /// Disable it if the session changes `NLS_DATE_LANGUAGE` to a non-English language.
    pub fn set_fast_dates(&mut self, value: bool) {
        self.fast_dates = value;
    }

    fn guard(&self, sql: &str) -> Result<(), OciError> {
        if self.read_only && !sqlt_core::readonly::is_read_only(sql) {
            return Err(OciError::ReadOnly);
        }
        Ok(())
    }

    pub fn query(&self, sql: &str, opts: QueryOptions) -> Result<QueryHandle, OciError> {
        self.query_with(sql, &[], opts)
    }

    pub fn query_with(
        &self,
        sql: &str,
        params: &[&dyn oracle::sql_type::ToSql],
        opts: QueryOptions,
    ) -> Result<QueryHandle, OciError> {
        self.guard(sql)?;
        let mut builder = self.conn.statement(sql);
        builder.fetch_array_size(opts.prefetch.max(1));
        if opts.lob_locator {
            builder.lob_locator();
        }
        let stmt = builder.build()?;
        let rs = stmt.into_result_set::<oracle::Row>(params)?;
        let columns = rs.column_info().iter().map(column_meta).collect::<Vec<_>>();
        Ok(QueryHandle {
            rs,
            conn: self.conn.clone(),
            columns,
            fast_dates: self.fast_dates,
            lob_locator: opts.lob_locator,
            lob_limit: opts.lob_limit,
        })
    }

    pub fn execute(&self, sql: &str) -> Result<u64, OciError> {
        self.execute_with(sql, &[])
    }

    pub fn execute_with(
        &self,
        sql: &str,
        params: &[&dyn oracle::sql_type::ToSql],
    ) -> Result<u64, OciError> {
        self.guard(sql)?;
        let stmt = self.conn.execute(sql, params)?;
        Ok(stmt.row_count()?)
    }

    /// `DBMS_OUTPUT.ENABLE(NULL)`; lines are collected by `dbms_output_lines`.
    pub fn enable_dbms_output(&self) -> Result<(), OciError> {
        self.execute("begin dbms_output.enable(null); end;")?;
        self.dbms_output_enabled
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    /// Drains the whole `DBMS_OUTPUT` buffer with `GET_LINE` (OUT binds),
    /// keeping at most `max` lines and appending a marker when the rest was
    /// truncated. Returns an empty vector when output was not enabled.
    ///
    /// Draining is auxiliary to the statement that produced the output, so an
    /// OCI error (for example `DBMS_OUTPUT.DISABLE` run inside the script,
    /// which purges the buffer and rejects `GET_LINE`) ends the drain instead of
    /// failing the script; the lines collected so far are returned.
    pub fn dbms_output_lines(&self, max: usize) -> Result<Vec<String>, OciError> {
        if !self
            .dbms_output_enabled
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return Ok(Vec::new());
        }
        let mut lines = Vec::new();
        let mut truncated = 0usize;
        loop {
            let Ok(mut stmt) = self
                .conn
                .statement("begin dbms_output.get_line(:1, :2); end;")
                .build()
            else {
                break;
            };
            // An OUT string bind needs a buffer; `None::<String>` is empty.
            if stmt
                .execute(&[&OracleType::Varchar2(32767), &None::<i32>])
                .is_err()
            {
                break;
            }
            // Status is 1 when the buffer is drained; the line is NULL then.
            let Ok(status) = stmt.bind_value::<_, i32>(2) else {
                break;
            };
            if status != 0 {
                break;
            }
            let Ok(line) = stmt.bind_value::<_, String>(1) else {
                break;
            };
            if lines.len() < max {
                lines.push(line);
            } else {
                truncated += 1;
            }
        }
        if truncated > 0 {
            lines.push(format!(
                "-- {truncated} more DBMS_OUTPUT lines were truncated"
            ));
        }
        Ok(lines)
    }

    pub fn commit(&self) -> Result<(), OciError> {
        self.conn.commit()?;
        Ok(())
    }

    pub fn rollback(&self) -> Result<(), OciError> {
        self.conn.rollback()?;
        Ok(())
    }

    /// Cancels the running execution (`OCIBreak`). Safe from another thread.
    pub fn cancel(&self) -> Result<(), OciError> {
        self.conn.break_execution()?;
        Ok(())
    }

    pub fn server_version(&self) -> Result<String, OciError> {
        let (_, banner) = self.conn.server_version()?;
        Ok(banner)
    }
}

impl QueryHandle {
    pub fn columns(&self) -> &[ColumnMeta] {
        &self.columns
    }

    /// Returns at most `max` rows. An empty batch means end of result.
    pub fn next_batch(&mut self, max: usize) -> Result<Batch, OciError> {
        let mut rows = Vec::new();
        for _ in 0..max {
            match self.rs.next() {
                Some(Ok(row)) => rows.push(map_row(
                    &row,
                    self.fast_dates,
                    self.lob_locator,
                    self.lob_limit,
                )?),
                Some(Err(e)) => return Err(e.into()),
                None => break,
            }
        }
        Ok(Batch {
            columns: self.columns.clone(),
            rows,
        })
    }

    /// Cancels the running execution (`OCIBreak`). The session remains usable.
    pub fn cancel(&self) -> Result<(), OciError> {
        self.conn.break_execution()?;
        Ok(())
    }
}

fn resolve_config_dir(spec: &ConnectSpec) -> Result<Option<std::path::PathBuf>, OciError> {
    let Some(folder) = &spec.tns_admin else {
        return Ok(None);
    };
    let ConnectTarget::Alias(alias) = &spec.target else {
        return Ok(Some(folder.clone()));
    };
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    let overlay_root = std::env::temp_dir().join("sqltools-tns");
    let plan = crate::wallet::prepare(folder, alias, &overlay_root, &home);
    if let Some(error) = plan.error {
        return Err(OciError::Wallet(error));
    }
    Ok(Some(plan.tns_admin))
}

fn session_uses_english_dates(conn: &Connection) -> bool {
    let sql = "select value from nls_session_parameters where parameter = 'NLS_DATE_LANGUAGE'";
    let Ok(row) = conn.query_row(sql, &[]) else {
        return false;
    };
    let Ok(value) = row.get::<_, String>(0) else {
        return false;
    };
    let value = value.to_ascii_uppercase();
    value.starts_with("AMERICAN") || value.starts_with("ENGLISH")
}

fn column_meta(info: &oracle::ColumnInfo) -> ColumnMeta {
    let ot = info.oracle_type();
    ColumnMeta {
        name: info.name().to_string(),
        type_name: ot.to_string(),
        nullable: info.nullable(),
        numeric: matches!(
            ot,
            OracleType::Number(_, _)
                | OracleType::Float(_)
                | OracleType::BinaryFloat
                | OracleType::BinaryDouble
        ),
    }
}

fn base_date_format() -> &'static DateFormat {
    static FMT: OnceLock<DateFormat> = OnceLock::new();
    FMT.get_or_init(|| {
        compile_date_format("YYYY-MM-DD HH24:MI:SS").expect("valid base date format")
    })
}

fn format_timestamp(t: &Timestamp, precision: u8, with_tz: bool) -> String {
    let mut out = base_date_format().format(
        t.year(),
        t.month(),
        t.day(),
        t.hour(),
        t.minute(),
        t.second(),
    );
    if precision > 0 {
        let frac = format!("{:09}", t.nanosecond());
        out.push('.');
        out.push_str(&frac[..usize::from(precision.min(9))]);
    }
    if with_tz {
        out.push_str(&format!(
            " {:+03}:{:02}",
            t.tz_hour_offset(),
            t.tz_minute_offset().abs()
        ));
    }
    out
}

fn read_lob(value: &oracle::SqlValue, limit: usize, nclob: bool) -> Result<String, OciError> {
    let mut buf = Vec::new();
    if nclob {
        let mut lob: Nclob = value.get()?;
        let mut reader = (&mut lob).take(limit as u64);
        reader
            .read_to_end(&mut buf)
            .map_err(|e| OciError::Io(e.to_string()))?;
    } else {
        let mut lob: Clob = value.get()?;
        let mut reader = (&mut lob).take(limit as u64);
        reader
            .read_to_end(&mut buf)
            .map_err(|e| OciError::Io(e.to_string()))?;
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn read_blob(value: &oracle::SqlValue, limit: usize) -> Result<Vec<u8>, OciError> {
    let mut lob: oracle::sql_type::Blob = value.get()?;
    let mut buf = Vec::new();
    let mut reader = (&mut lob).take(limit as u64);
    reader
        .read_to_end(&mut buf)
        .map_err(|e| OciError::Io(e.to_string()))?;
    Ok(buf)
}

fn map_value(
    value: &oracle::SqlValue,
    info: &oracle::ColumnInfo,
    fast_dates: bool,
    lob_locator: bool,
    lob_limit: usize,
) -> Result<Value, OciError> {
    if value.is_null()? {
        return Ok(Value::Null);
    }
    let ot = info.oracle_type();
    match ot {
        OracleType::Varchar2(_)
        | OracleType::NVarchar2(_)
        | OracleType::Char(_)
        | OracleType::NChar(_)
        | OracleType::Long
        | OracleType::Rowid
        | OracleType::Json
        | OracleType::Xml => Ok(Value::Text(value.get::<String>()?)),
        OracleType::CLOB => {
            if lob_locator {
                Ok(Value::Text(read_lob(value, lob_limit, false)?))
            } else {
                Ok(Value::Text(value.get::<String>()?))
            }
        }
        OracleType::NCLOB => {
            if lob_locator {
                Ok(Value::Text(read_lob(value, lob_limit, true)?))
            } else {
                Ok(Value::Text(value.get::<String>()?))
            }
        }
        OracleType::Number(_, _)
        | OracleType::Float(_)
        | OracleType::BinaryFloat
        | OracleType::BinaryDouble => Ok(Value::Number(value.get::<String>()?)),
        OracleType::Date => {
            let t: Timestamp = value.get()?;
            if fast_dates {
                Ok(Value::Date(format_timestamp(&t, 0, false)))
            } else {
                Ok(Value::Date(
                    value
                        .get::<String>()
                        .unwrap_or_else(|_| format_timestamp(&t, 0, false)),
                ))
            }
        }
        OracleType::Timestamp(prec) => {
            let t: Timestamp = value.get()?;
            if fast_dates {
                Ok(Value::Timestamp(format_timestamp(&t, *prec, false)))
            } else {
                Ok(Value::Timestamp(
                    value
                        .get::<String>()
                        .unwrap_or_else(|_| format_timestamp(&t, *prec, false)),
                ))
            }
        }
        OracleType::TimestampTZ(prec) | OracleType::TimestampLTZ(prec) => {
            let t: Timestamp = value.get()?;
            if fast_dates {
                Ok(Value::Timestamp(format_timestamp(&t, *prec, true)))
            } else {
                Ok(Value::Timestamp(
                    value
                        .get::<String>()
                        .unwrap_or_else(|_| format_timestamp(&t, *prec, true)),
                ))
            }
        }
        OracleType::IntervalDS(_, _) => {
            let interval: IntervalDS = value.get()?;
            Ok(Value::Text(interval.to_string()))
        }
        OracleType::IntervalYM(_) => {
            let interval: IntervalYM = value.get()?;
            Ok(Value::Text(interval.to_string()))
        }
        OracleType::Raw(_) | OracleType::LongRaw => Ok(Value::Binary(value.get::<Vec<u8>>()?)),
        OracleType::BLOB => {
            if lob_locator {
                Ok(Value::Binary(read_blob(value, lob_limit)?))
            } else {
                Ok(Value::Binary(value.get::<Vec<u8>>()?))
            }
        }
        other => Err(OciError::Unsupported(format!(
            "Oracle type not supported yet: {other}"
        ))),
    }
}

fn map_row(
    row: &oracle::Row,
    fast_dates: bool,
    lob_locator: bool,
    lob_limit: usize,
) -> Result<Vec<Value>, OciError> {
    row.sql_values()
        .iter()
        .zip(row.column_info())
        .map(|(value, info)| map_value(value, info, fast_dates, lob_locator, lob_limit))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_wallet_returns_wallet_error() {
        let dir = std::env::temp_dir().join(format!("sqlt-session-wallet-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("tnsnames.ora"),
            "db_high = (description=(address=(protocol=tcps)(port=1522)(host=h))\
             (connect_data=(service_name=s))\
             (security=(my_wallet_directory= C:\\Wallet_Missing_Sqlt)))\n",
        )
        .unwrap();
        let spec = ConnectSpec {
            user: "u".to_string(),
            password: "p".to_string(),
            target: ConnectTarget::Alias("db_high".to_string()),
            mode: ConnectMode::Normal,
            read_only: false,
            oci_dir: None,
            tns_admin: Some(dir.clone()),
            prefetch: 1000,
        };
        let error = resolve_config_dir(&spec).unwrap_err();
        assert!(matches!(error, OciError::Wallet(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
