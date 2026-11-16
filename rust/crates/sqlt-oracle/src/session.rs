// SPDX-License-Identifier: GPL-3.0-only
//! Sesión Oracle: conexión, consultas por lotes, cancelación y modo de solo lectura.

use std::sync::{Arc, OnceLock};

use oracle::sql_type::{OracleType, Timestamp};
use oracle::{Connection, Connector, Privilege};
use sqlt_core::datefmt::{compile_date_format, DateFormat};
use sqlt_core::{Batch, ColumnMeta, Value};

use crate::oci::{locate_oci_dir, prepare_oci_environment, OciError};
use crate::spec::{ConnectMode, ConnectSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryOptions {
    pub prefetch: u32,
}

impl Default for QueryOptions {
    fn default() -> Self {
        QueryOptions { prefetch: 1000 }
    }
}

pub struct Session {
    conn: Arc<Connection>,
    read_only: bool,
}

pub struct QueryHandle {
    rs: oracle::ResultSet<'static, oracle::Row>,
    conn: Arc<Connection>,
    columns: Vec<ColumnMeta>,
}

impl Session {
    pub fn open(spec: &ConnectSpec) -> Result<Session, OciError> {
        let lib_dir = match &spec.oci_dir {
            Some(dir) => dir.clone(),
            None => locate_oci_dir()?,
        };
        prepare_oci_environment(&lib_dir, spec.tns_admin.as_deref())?;

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
        Ok(Session {
            conn: Arc::new(conn),
            read_only: spec.read_only,
        })
    }

    fn guard(&self, sql: &str) -> Result<(), OciError> {
        if self.read_only && !sqlt_core::readonly::is_read_only(sql) {
            return Err(OciError::ReadOnly);
        }
        Ok(())
    }

    pub fn query(&self, sql: &str, opts: QueryOptions) -> Result<QueryHandle, OciError> {
        self.guard(sql)?;
        let mut builder = self.conn.statement(sql);
        builder.fetch_array_size(opts.prefetch.max(1));
        let stmt = builder.build()?;
        let rs = stmt.into_result_set::<oracle::Row>(&[])?;
        let columns = rs.column_info().iter().map(column_meta).collect::<Vec<_>>();
        Ok(QueryHandle {
            rs,
            conn: self.conn.clone(),
            columns,
        })
    }

    pub fn execute(&self, sql: &str) -> Result<u64, OciError> {
        self.guard(sql)?;
        let stmt = self.conn.execute(sql, &[])?;
        Ok(stmt.row_count()?)
    }

    pub fn commit(&self) -> Result<(), OciError> {
        self.conn.commit()?;
        Ok(())
    }

    pub fn rollback(&self) -> Result<(), OciError> {
        self.conn.rollback()?;
        Ok(())
    }

    /// Cancela la ejecución en curso (`OCIBreak`). Seguro desde otro hilo.
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

    /// Devuelve como máximo `max` filas. Un lote vacío indica fin del resultado.
    pub fn next_batch(&mut self, max: usize) -> Result<Batch, OciError> {
        let mut rows = Vec::new();
        for _ in 0..max {
            match self.rs.next() {
                Some(Ok(row)) => rows.push(map_row(&row)?),
                Some(Err(e)) => return Err(e.into()),
                None => break,
            }
        }
        Ok(Batch {
            columns: self.columns.clone(),
            rows,
        })
    }

    /// Cancela la ejecución en curso (`OCIBreak`). La sesión sigue usable.
    pub fn cancel(&self) -> Result<(), OciError> {
        self.conn.break_execution()?;
        Ok(())
    }
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
        compile_date_format("YYYY-MM-DD HH24:MI:SS").expect("formato de fecha base válido")
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

fn map_value(value: &oracle::SqlValue, info: &oracle::ColumnInfo) -> Result<Value, OciError> {
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
        | OracleType::CLOB
        | OracleType::NCLOB => Ok(Value::Text(value.get::<String>()?)),
        OracleType::Number(_, _)
        | OracleType::Float(_)
        | OracleType::BinaryFloat
        | OracleType::BinaryDouble => Ok(Value::Number(value.get::<String>()?)),
        OracleType::Date => {
            let t: Timestamp = value.get()?;
            Ok(Value::Date(format_timestamp(&t, 0, false)))
        }
        OracleType::Timestamp(prec) => {
            let t: Timestamp = value.get()?;
            Ok(Value::Timestamp(format_timestamp(&t, *prec, false)))
        }
        OracleType::TimestampTZ(prec) | OracleType::TimestampLTZ(prec) => {
            let t: Timestamp = value.get()?;
            Ok(Value::Timestamp(format_timestamp(&t, *prec, true)))
        }
        OracleType::Raw(_) | OracleType::LongRaw | OracleType::BLOB => {
            Ok(Value::Binary(value.get::<Vec<u8>>()?))
        }
        other => Err(OciError::Unsupported(format!(
            "Tipo Oracle no soportado todavía: {other}"
        ))),
    }
}

fn map_row(row: &oracle::Row) -> Result<Vec<Value>, OciError> {
    row.sql_values()
        .iter()
        .zip(row.column_info())
        .map(|(value, info)| map_value(value, info))
        .collect()
}
