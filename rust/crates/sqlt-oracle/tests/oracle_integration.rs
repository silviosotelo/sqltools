// SPDX-License-Identifier: GPL-3.0-only
//! Pruebas de integración contra Oracle real. Se saltan si faltan
//! `SQLT_USER`/`SQLT_PASS` y `SQLT_ALIAS`/`SQLT_CONNECT`.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use sqlt_core::{Batch, Value};
use sqlt_oracle::{ConnectSpec, OciError, QueryOptions, Session};

fn spec() -> Option<ConnectSpec> {
    match ConnectSpec::from_env() {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("SKIP integración Oracle: {e}");
            None
        }
    }
}

fn open(spec: &ConnectSpec) -> Session {
    Session::open(spec).unwrap_or_else(|e| panic!("no se pudo conectar: {e}"))
}

#[test]
fn connect_y_select_dual() {
    let Some(spec) = spec() else { return };
    let session = open(&spec);
    let mut q = session
        .query("select 1 from dual", QueryOptions::default())
        .expect("query");
    assert_eq!(q.columns().len(), 1);
    let batch = q.next_batch(10).expect("fetch");
    assert_eq!(batch.rows.len(), 1);
    assert_eq!(batch.rows[0][0], Value::Number("1".to_string()));
    assert!(q.next_batch(10).expect("fin").rows.is_empty());
}

#[test]
fn tipos_comunes() {
    let Some(spec) = spec() else { return };
    let session = open(&spec);
    let sql = "select 1 n, 1.5 d, 'ñandú' t, date '2024-02-29' f, \
               timestamp '2024-02-29 13:45:09.123' ts, cast(null as varchar2(10)) z from dual";
    let mut q = session.query(sql, QueryOptions::default()).expect("query");
    let batch = q.next_batch(10).expect("fetch");
    assert_eq!(batch.rows.len(), 1);
    let row = &batch.rows[0];
    assert_eq!(row[0], Value::Number("1".to_string()));
    assert_eq!(row[1], Value::Number("1.5".to_string()));
    assert_eq!(row[2], Value::Text("ñandú".to_string()));
    assert_eq!(row[3], Value::Date("2024-02-29 00:00:00".to_string()));
    match &row[4] {
        Value::Timestamp(s) => assert!(s.starts_with("2024-02-29 13:45:09.123"), "{s}"),
        other => panic!("timestamp inesperado: {other:?}"),
    }
    assert_eq!(row[5], Value::Null);
}

#[test]
fn emoji_solo_si_el_charset_es_utf8() {
    let Some(spec) = spec() else { return };
    let session = open(&spec);
    let mut q = session
        .query(
            "select value from nls_database_parameters where parameter = 'NLS_CHARACTERSET'",
            QueryOptions::default(),
        )
        .expect("query charset");
    let batch = q.next_batch(1).expect("fetch");
    let charset = batch.rows[0][0].display().unwrap_or_default().to_string();
    if charset != "AL32UTF8" {
        eprintln!("SKIP emoji: el charset de la base es {charset}");
        return;
    }
    let mut q = session
        .query("select 'ñ😀' from dual", QueryOptions::default())
        .expect("query emoji");
    let batch = q.next_batch(1).expect("fetch");
    assert_eq!(batch.rows[0][0], Value::Text("ñ😀".to_string()));
}

#[test]
fn cancelar_consulta_larga_y_reutilizar_sesion() {
    let Some(spec) = spec() else { return };
    let session = Arc::new(open(&spec));
    let worker = session.clone();
    let handle = thread::spawn(move || -> Result<Batch, OciError> {
        let mut q = worker.query(
            "select count(*) from dual connect by level <= 100000000",
            QueryOptions::default(),
        )?;
        q.next_batch(1)
    });
    thread::sleep(Duration::from_millis(500));
    session.cancel().expect("break");
    let result = handle.join().expect("join");
    let err = result.expect_err("la consulta debía cancelarse");
    let msg = err.to_string();
    assert!(msg.contains("ORA-01013"), "error inesperado: {msg}");

    let mut q = session
        .query("select 1 from dual", QueryOptions::default())
        .expect("sesión reutilizable");
    assert_eq!(q.next_batch(1).expect("fetch").rows.len(), 1);
}

#[test]
fn lotes_respetan_el_maximo() {
    let Some(spec) = spec() else { return };
    let session = open(&spec);
    let mut q = session
        .query(
            "select level from dual connect by level <= 100000",
            QueryOptions { prefetch: 5000 },
        )
        .expect("query");
    let mut total = 0usize;
    loop {
        let batch = q.next_batch(1000).expect("fetch");
        assert!(batch.rows.len() <= 1000);
        if batch.rows.is_empty() {
            break;
        }
        total += batch.rows.len();
    }
    assert_eq!(total, 100_000);
}

#[test]
fn modo_solo_lectura_bloquea_escrituras() {
    let Some(mut spec) = spec() else { return };
    spec.read_only = true;
    let session = open(&spec);
    assert!(session
        .query("select 1 from dual", QueryOptions::default())
        .is_ok());
    assert!(matches!(
        session.execute("delete from dual"),
        Err(OciError::ReadOnly)
    ));
}
