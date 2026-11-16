// SPDX-License-Identifier: GPL-3.0-only
//! Headless SQLTools Rust harness: diagnostics, queries and benchmarks.

mod args;

use std::time::Instant;

use args::{parse, Command, USAGE};
use sqlt_core::{Batch, Value};
use sqlt_oracle::{locate_oci_dir, ConnectSpec, QueryOptions, Session};

fn display(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Binary(bytes) => bytes.iter().map(|b| format!("{b:02X}")).collect(),
        other => other.display().unwrap_or_default().to_string(),
    }
}

fn print_batch(batch: &Batch, header: &mut bool) {
    if *header {
        let names: Vec<&str> = batch.columns.iter().map(|c| c.name.as_str()).collect();
        println!("{}", names.join("\t"));
        *header = false;
    }
    for row in &batch.rows {
        let line: Vec<String> = row.iter().map(display).collect();
        println!("{}", line.join("\t"));
    }
}

fn run_query(
    spec: &ConnectSpec,
    sql: &str,
    max_rows: usize,
    prefetch: u32,
    with_header: bool,
) -> Result<(), String> {
    let session = Session::open(spec).map_err(|e| e.to_string())?;
    let mut handle = session
        .query(
            sql,
            QueryOptions {
                prefetch,
                ..QueryOptions::default()
            },
        )
        .map_err(|e| e.to_string())?;
    let mut header = with_header;
    let mut printed = 0usize;
    loop {
        let want = if max_rows == 0 {
            1000
        } else {
            (max_rows - printed).min(1000)
        };
        if want == 0 {
            break;
        }
        let batch = handle.next_batch(want).map_err(|e| e.to_string())?;
        if batch.rows.is_empty() {
            if header {
                let names: Vec<&str> = batch.columns.iter().map(|c| c.name.as_str()).collect();
                println!("{}", names.join("\t"));
            }
            break;
        }
        print_batch(&batch, &mut header);
        printed += batch.rows.len();
    }
    eprintln!("{printed} rows");
    Ok(())
}

fn run_bench(spec: &ConnectSpec, sql: &str, runs: usize, prefetch: u32) -> Result<(), String> {
    let session = Session::open(spec).map_err(|e| e.to_string())?;
    let start = Instant::now();
    let mut total = 0usize;
    for _ in 0..runs {
        let mut handle = session
            .query(
                sql,
                QueryOptions {
                    prefetch,
                    ..QueryOptions::default()
                },
            )
            .map_err(|e| e.to_string())?;
        loop {
            let batch = handle.next_batch(1000).map_err(|e| e.to_string())?;
            if batch.rows.is_empty() {
                break;
            }
            total += batch.rows.len();
        }
    }
    let secs = start.elapsed().as_secs_f64();
    if secs > 0.0 {
        println!(
            "{total} rows in {secs:.3} s ({:.0} rows/s, {runs} runs)",
            total as f64 / secs
        );
    }
    Ok(())
}

fn run_ddl(spec: &ConnectSpec, schema: Option<&str>, table: &str) -> Result<(), String> {
    let session = Session::open(spec).map_err(|e| e.to_string())?;
    let ddl = sqlt_meta::loader::load_table_ddl(&session, schema, table, QueryOptions::default())
        .map_err(|e| e.to_string())?;
    print!("{ddl}");
    Ok(())
}

fn run_compare(
    spec: &ConnectSpec,
    path: &str,
    schema: Option<&str>,
    table: &str,
) -> Result<(), String> {
    let expected = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let session = Session::open(spec).map_err(|e| e.to_string())?;
    let actual =
        sqlt_meta::loader::load_table_ddl(&session, schema, table, QueryOptions::default())
            .map_err(|e| e.to_string())?;
    let comparison = sqlt_meta::compare_ddl(&expected, &actual);
    println!("{}", comparison.report());
    if !comparison.identical {
        std::process::exit(1);
    }
    Ok(())
}

fn run_diag() -> Result<(), String> {
    match locate_oci_dir() {
        Ok(dir) => println!("OCI dir: {}", dir.display()),
        Err(e) => println!("OCI dir: ERROR: {e}"),
    }
    println!(
        "TNS_ADMIN: {}",
        std::env::var("SQLT_TNS_ADMIN").unwrap_or_else(|_| "(not set)".to_string())
    );
    match ConnectSpec::from_env() {
        Ok(spec) => match Session::open(&spec) {
            Ok(session) => match session.server_version() {
                Ok(version) => println!("Server: {version}"),
                Err(e) => println!("Server: ERROR: {e}"),
            },
            Err(e) => println!("Connection: ERROR: {e}"),
        },
        Err(e) => println!("Connection: SKIP: {e}"),
    }
    Ok(())
}

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let result = match parse(&argv) {
        Ok(Command::Help) => {
            println!("{USAGE}");
            Ok(())
        }
        Ok(Command::Diag) => run_diag(),
        Ok(Command::Query {
            sql,
            max_rows,
            prefetch,
            header,
        }) => ConnectSpec::from_env()
            .map_err(|e| e.to_string())
            .and_then(|spec| run_query(&spec, &sql, max_rows, prefetch, header)),
        Ok(Command::Bench {
            sql,
            runs,
            prefetch,
        }) => ConnectSpec::from_env()
            .map_err(|e| e.to_string())
            .and_then(|spec| run_bench(&spec, &sql, runs, prefetch)),
        Ok(Command::Ddl { schema, table }) => ConnectSpec::from_env()
            .map_err(|e| e.to_string())
            .and_then(|spec| run_ddl(&spec, schema.as_deref(), &table)),
        Ok(Command::CompareDdl {
            path,
            schema,
            table,
        }) => ConnectSpec::from_env()
            .map_err(|e| e.to_string())
            .and_then(|spec| run_compare(&spec, &path, schema.as_deref(), &table)),
        Err(e) => Err(e),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
