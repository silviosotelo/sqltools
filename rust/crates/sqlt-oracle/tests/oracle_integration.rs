// SPDX-License-Identifier: GPL-3.0-only
//! Integration tests against a real Oracle database. They are skipped when
//! `SQLT_USER`/`SQLT_PASS` and `SQLT_ALIAS`/`SQLT_CONNECT` are missing.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use sqlt_core::{Batch, Value};
use sqlt_oracle::{ConnectSpec, OciError, QueryOptions, Session};

fn spec() -> Option<ConnectSpec> {
    match ConnectSpec::from_env() {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("SKIP Oracle integration: {e}");
            None
        }
    }
}

fn open(spec: &ConnectSpec) -> Session {
    Session::open(spec).unwrap_or_else(|e| panic!("could not connect: {e}"))
}

#[test]
fn connect_and_select_dual() {
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
fn common_types() {
    let Some(spec) = spec() else { return };
    let session = open(&spec);
    let sql = "select 1 n, 1.5 d, 'áéíóú' t, date '2024-02-29' f, \
               timestamp '2024-02-29 13:45:09.123' ts, cast(null as varchar2(10)) z from dual";
    let mut q = session.query(sql, QueryOptions::default()).expect("query");
    let batch = q.next_batch(10).expect("fetch");
    assert_eq!(batch.rows.len(), 1);
    let row = &batch.rows[0];
    assert_eq!(row[0], Value::Number("1".to_string()));
    assert_eq!(row[1], Value::Number("1.5".to_string()));
    assert_eq!(row[2], Value::Text("áéíóú".to_string()));
    assert_eq!(row[3], Value::Date("2024-02-29 00:00:00".to_string()));
    match &row[4] {
        Value::Timestamp(s) => assert!(s.starts_with("2024-02-29 13:45:09.123"), "{s}"),
        other => panic!("unexpected timestamp: {other:?}"),
    }
    assert_eq!(row[5], Value::Null);
}

#[test]
fn emoji_only_if_charset_is_utf8() {
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
fn cancel_long_query_and_reuse_session() {
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
    let err = result.expect_err("the query should have been cancelled");
    let msg = err.to_string();
    assert!(msg.contains("ORA-01013"), "unexpected error: {msg}");

    let mut q = session
        .query("select 1 from dual", QueryOptions::default())
        .expect("reusable session");
    assert_eq!(q.next_batch(1).expect("fetch").rows.len(), 1);
}

#[test]
fn batches_respect_max() {
    let Some(spec) = spec() else { return };
    let session = open(&spec);
    let mut q = session
        .query(
            "select level from dual connect by level <= 100000",
            QueryOptions {
                prefetch: 5000,
                ..QueryOptions::default()
            },
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
fn lobs_by_locator() {
    let Some(spec) = spec() else { return };
    let session = open(&spec);
    let sql = "select to_clob('clob ñ'), to_nclob('nclob ñ'), \
               to_blob(hextoraw('0102030405')) from dual";
    let mut q = session
        .query(sql, QueryOptions::default())
        .expect("query lobs");
    let batch = q.next_batch(1).expect("fetch lobs");
    assert_eq!(batch.rows.len(), 1);
    let row = &batch.rows[0];
    assert_eq!(row[0], Value::Text("clob ñ".to_string()));
    assert_eq!(row[1], Value::Text("nclob ñ".to_string()));
    assert_eq!(row[2], Value::Binary(vec![1, 2, 3, 4, 5]));
}

#[test]
fn large_clob_read_by_locator() {
    let Some(spec) = spec() else { return };
    let session = open(&spec);
    let sql = "select to_clob(rpad('c', 4000, 'c')) || to_clob(rpad('d', 4000, 'd')) from dual";
    let mut q = session
        .query(sql, QueryOptions::default())
        .expect("query clob");
    let batch = q.next_batch(1).expect("fetch clob");
    let text = batch.rows[0][0].display().expect("clob texto");
    assert_eq!(text.len(), 8000);
    assert!(text.starts_with('c'));
    assert!(text.ends_with('d'));
}

#[test]
fn real_binds() {
    let Some(spec) = spec() else { return };
    let session = open(&spec);

    let mut q = session
        .query_with("select :t from dual", &[&"hola"], QueryOptions::default())
        .expect("bind texto");
    assert_eq!(
        q.next_batch(1).expect("fetch").rows[0][0].clone(),
        Value::Text("hola".to_string())
    );

    let mut q = session
        .query_with(
            "select :n + 1, :f * 2 from dual",
            &[&41i64, &1.25f64],
            QueryOptions::default(),
        )
        .expect("bind numbers");
    let batch = q.next_batch(1).expect("fetch");
    assert_eq!(batch.rows[0][0], Value::Number("42".to_string()));
    assert_eq!(batch.rows[0][1], Value::Number("2.5".to_string()));

    let mut q = session
        .query_with(
            "select nvl(:v, 'era nulo') from dual",
            &[&None::<String>],
            QueryOptions::default(),
        )
        .expect("bind null");
    assert_eq!(
        q.next_batch(1).expect("fetch").rows[0][0],
        Value::Text("era nulo".to_string())
    );

    let mut q = session
        .query_with(
            "select to_char(to_date(:s,'YYYY-MM-DD HH24:MI:SS') + 1, 'YYYY-MM-DD HH24:MI:SS') from dual",
            &[&"2026-12-31 23:59:59"],
            QueryOptions::default(),
        )
        .expect("bind fecha");
    assert_eq!(
        q.next_batch(1).expect("fetch").rows[0][0],
        Value::Text("2027-01-01 23:59:59".to_string())
    );

    let mut q = session
        .query_with(
            "select :a * 10 from dual",
            &[&7i64],
            QueryOptions::default(),
        )
        .expect("bind repetido");
    assert_eq!(
        q.next_batch(1).expect("fetch").rows[0][0],
        Value::Number("70".to_string())
    );

    let err = match session.query_with(
        "select to_number(:x) from dual",
        &[&"abc"],
        QueryOptions::default(),
    ) {
        Ok(_) => panic!("se esperaba ORA-01722"),
        Err(e) => e,
    };
    assert!(err.to_string().contains("ORA-01722"), "{err}");
}

#[test]
fn intervals() {
    let Some(spec) = spec() else { return };
    let session = open(&spec);
    let sql =
        "select interval '3-5' year to month, interval '2 03:04:05.678' day to second from dual";
    let mut q = session
        .query(sql, QueryOptions::default())
        .expect("query intervalos");
    let batch = q.next_batch(1).expect("fetch");
    let ym = batch.rows[0][0].display().unwrap_or_default();
    let ds = batch.rows[0][1].display().unwrap_or_default();
    assert!(ym.contains('3') && ym.contains('5'), "{ym}");
    assert!(ds.contains('2') && ds.contains("03"), "{ds}");
}

#[test]
fn nls_spanish_uses_driver_format() {
    let Some(spec) = spec() else { return };
    let mut session = open(&spec);
    session
        .execute("alter session set nls_date_language = 'SPANISH'")
        .expect("nls language");
    session
        .execute("alter session set nls_date_format = 'DD-MON-RR'")
        .expect("nls format");
    session.set_fast_dates(false);
    let mut q = session
        .query(
            "select date '2024-01-15' from dual",
            QueryOptions::default(),
        )
        .expect("query fecha");
    let batch = q.next_batch(1).expect("fetch");
    let text = batch.rows[0][0].display().unwrap_or_default().to_string();
    eprintln!("date with Spanish NLS: {text}");
    assert!(!text.is_empty());
}

#[test]
fn twenty_five_columns() {
    let Some(spec) = spec() else { return };
    let session = open(&spec);
    let sql = "select 42, -7, 3.14159, 0.5, \
               12345678901234567890123456789012345678, \
               cast(1234.5 as number(10,2)), \
               cast(1.5 as binary_float), cast(2.25 as binary_double), \
               'hola', cast('ab' as char(10)), \
               cast('ñ' as nvarchar2(20)), cast('ñ' as nchar(3)), \
               to_clob('texto clob'), to_nclob('nclob'), \
               to_blob(hextoraw('DEADBEEF')), hextoraw('DEADBEEF'), \
               rowid, '{\"k\":\"v\"}', \
               date '2024-02-29', \
               cast(timestamp '2024-02-29 13:45:09.123456' as timestamp(6)) \
               from dual";
    let mut q = session
        .query(sql, QueryOptions::default())
        .expect("query tipos");
    let batch = q.next_batch(1).expect("fetch tipos");
    let row = &batch.rows[0];
    eprintln!("tipos: {row:?}");
    assert_eq!(row[0], Value::Number("42".to_string()));
    assert_eq!(row[1], Value::Number("-7".to_string()));
    assert_eq!(row[2], Value::Number("3.14159".to_string()));
    assert!(matches!(&row[3], Value::Number(v) if v == ".5" || v == "0.5"));
    assert_eq!(
        row[4],
        Value::Number("12345678901234567890123456789012345678".to_string())
    );
    assert_eq!(row[5], Value::Number("1234.5".to_string()));
    assert_eq!(row[6], Value::Number("1.5".to_string()));
    assert_eq!(row[7], Value::Number("2.25".to_string()));
    assert_eq!(row[8], Value::Text("hola".to_string()));
    assert_eq!(row[9], Value::Text("ab        ".to_string()));
    assert_eq!(row[10], Value::Text("ñ".to_string()));
    assert_eq!(row[11], Value::Text("ñ  ".to_string()));
    assert_eq!(row[12], Value::Text("texto clob".to_string()));
    assert_eq!(row[13], Value::Text("nclob".to_string()));
    assert_eq!(row[14], Value::Binary(vec![0xDE, 0xAD, 0xBE, 0xEF]));
    assert_eq!(row[15], Value::Binary(vec![0xDE, 0xAD, 0xBE, 0xEF]));
    assert!(matches!(&row[16], Value::Text(v) if !v.is_empty()));
    assert!(matches!(&row[17], Value::Text(v) if v.contains("\"k\"")));
    assert_eq!(row[18], Value::Date("2024-02-29 00:00:00".to_string()));
    assert!(matches!(&row[19], Value::Timestamp(v) if v.starts_with("2024-02-29 13:45:09.123456")));
}

#[test]
fn extreme_and_informative_numbers() {
    let Some(spec) = spec() else { return };
    let session = open(&spec);
    let sql = "select 1e-10, 1e100, binary_double_nan, binary_double_infinity from dual";
    let mut q = session
        .query(sql, QueryOptions::default())
        .expect("query extremes");
    let batch = q.next_batch(1).expect("fetch extremes");
    eprintln!("extremes: {:?}", batch.rows[0]);
    assert_eq!(batch.rows[0].len(), 4);
}

#[test]
fn xmltype_by_support() {
    let Some(spec) = spec() else { return };
    let session = open(&spec);
    match session.query(
        "select xmltype('<a><b>1</b></a>') from dual",
        QueryOptions::default(),
    ) {
        Ok(mut q) => {
            let batch = q.next_batch(1).expect("fetch xml");
            eprintln!("xmltype: {:?}", batch.rows[0]);
            assert_eq!(batch.rows[0].len(), 1);
        }
        Err(err) => {
            eprintln!("xmltype unsupported: {err}");
            assert!(matches!(err, OciError::Unsupported(_)));
        }
    }
}

#[test]
fn read_only_blocks_writes() {
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

fn parse_iso(s: &str) -> Option<(i32, u32, u32, u32, u32, u32)> {
    let b = s.as_bytes();
    if b.len() < 19
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b' '
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }
    Some((
        s[0..4].parse().ok()?,
        s[5..7].parse().ok()?,
        s[8..10].parse().ok()?,
        s[11..13].parse().ok()?,
        s[14..16].parse().ok()?,
        s[17..19].parse().ok()?,
    ))
}

#[test]
#[ignore = "slow golden: 260,000 dates against TO_CHAR"]
fn golden_dates_260k() {
    use sqlt_core::datefmt::compile_date_format;
    let Some(spec) = spec() else { return };
    let session = open(&spec);
    session
        .execute("alter session set nls_date_language = 'AMERICAN'")
        .expect("nls american");
    let formats = [
        "DD-MON-RR",
        "DD-MON-YYYY",
        "YYYY-MM-DD HH24:MI:SS",
        "YYYY-MM-DD",
        "DD/MM/YYYY",
        "DD/MM/YYYY HH24:MI:SS",
        "DD-MON-RR HH.MI.SS AM",
        "MM/DD/YYYY HH:MI:SS AM",
        "Day, Month DD, YYYY",
        "DY DD Mon YYYY",
        "DAY DD-MON-YY",
        "YYYY-MM-DD\"T\"HH24:MI:SS",
        "DDD YYYY Q",
    ];
    let mut total_fail = 0usize;
    for fmt in formats {
        let df = compile_date_format(fmt).unwrap_or_else(|| panic!("no soportado: {fmt}"));
        let sql = format!(
            "select to_char(d,'YYYY-MM-DD HH24:MI:SS'), to_char(d,'{fmt}') from ( \
             select date '1900-01-01' + trunc(dbms_random.value(0,73000)) + trunc(dbms_random.value(0,86400))/86400 d \
             from dual connect by level <= 20000 \
             union all select date '2024-02-29' from dual \
             union all select date '1900-03-01' from dual \
             union all select date '2000-12-31' + 23/24 + 59/1440 + 59/86400 from dual \
             union all select date '2026-01-01' from dual)"
        );
        let mut q = session
            .query(&sql, QueryOptions::default())
            .expect("query golden");
        let mut total = 0usize;
        let mut bad = 0usize;
        loop {
            let batch = q.next_batch(1000).expect("fetch golden");
            if batch.rows.is_empty() {
                break;
            }
            for row in &batch.rows {
                let iso = row[0].display().unwrap_or_default();
                let want = row[1].display().unwrap_or_default();
                let Some((y, mo, d, h, mi, s)) = parse_iso(iso) else {
                    continue;
                };
                let got = df.format(y, mo, d, h, mi, s);
                total += 1;
                if got != want {
                    bad += 1;
                    if bad <= 3 {
                        eprintln!("{fmt}: {iso} oracle=[{want}] ours=[{got}]");
                    }
                }
            }
        }
        eprintln!("{fmt}: {total} dates, {bad} differences");
        assert_eq!(bad, 0, "{fmt}");
        total_fail += bad;
    }
    eprintln!("golden dates: {} differences", total_fail);
}

#[test]
fn dbms_output_uses_out_binds() {
    let Some(spec) = spec() else { return };
    let session = open(&spec);
    assert!(session
        .dbms_output_lines(10)
        .expect("buffer before enable")
        .is_empty());
    session.enable_dbms_output().expect("enable dbms_output");
    session
        .execute("begin dbms_output.put_line('first'); dbms_output.put_line('second'); end;")
        .expect("put_line");
    let lines = session.dbms_output_lines(10).expect("get_line");
    assert_eq!(lines, vec!["first".to_string(), "second".to_string()]);
    assert!(session
        .dbms_output_lines(10)
        .expect("drained buffer")
        .is_empty());
}

#[test]
fn xmltype_maps_to_text() {
    let Some(spec) = spec() else { return };
    let session = open(&spec);
    let mut q = session
        .query(
            "select xmltype('<root><item>42</item></root>') from dual",
            QueryOptions::default(),
        )
        .expect("query xmltype");
    let batch = q.next_batch(10).expect("fetch xmltype");
    match &batch.rows[0][0] {
        Value::Text(text) => assert!(text.contains("<item>42</item>"), "{text}"),
        other => panic!("unexpected xmltype value: {other:?}"),
    }
}
