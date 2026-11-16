// SPDX-License-Identifier: GPL-3.0-only
//! CLI argument parsing. No external dependencies.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    Diag,
    Query {
        sql: String,
        max_rows: usize,
        prefetch: u32,
        header: bool,
    },
    Bench {
        sql: String,
        runs: usize,
        prefetch: u32,
    },
}

pub const USAGE: &str = "\
usage:
  sqlt-cli diag
  sqlt-cli query \"<sql>\" [--max-rows N] [--prefetch N] [--no-header]
  sqlt-cli bench \"<sql>\" [--runs N] [--prefetch N]

Environment: SQLT_USER, SQLT_PASS, SQLT_ALIAS or SQLT_CONNECT,
             SQLT_OCI_DIR, SQLT_TNS_ADMIN, SQLT_MODE, SQLT_READ_ONLY, SQLT_PREFETCH";

fn parse_u32(flag: &str, value: Option<&String>) -> Result<u32, String> {
    let v = value.ok_or_else(|| format!("missing value for {flag}"))?;
    v.parse::<u32>()
        .map_err(|_| format!("invalid value for {flag}: {v}"))
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    let Some(cmd) = args.first() else {
        return Err(USAGE.to_string());
    };
    match cmd.as_str() {
        "-h" | "--help" | "help" => Ok(Command::Help),
        "diag" => {
            if args.len() > 1 {
                return Err("diag does not accept arguments".to_string());
            }
            Ok(Command::Diag)
        }
        "query" | "bench" => {
            let mut sql_parts: Vec<String> = Vec::new();
            let mut max_rows = 1000usize;
            let mut runs = 3usize;
            let mut prefetch = 1000u32;
            let mut header = true;
            let mut i = 1usize;
            while i < args.len() {
                match args[i].as_str() {
                    "--max-rows" => {
                        max_rows = parse_u32("--max-rows", args.get(i + 1))? as usize;
                        i += 2;
                    }
                    "--runs" => {
                        runs = parse_u32("--runs", args.get(i + 1))? as usize;
                        i += 2;
                    }
                    "--prefetch" => {
                        prefetch = parse_u32("--prefetch", args.get(i + 1))?;
                        i += 2;
                    }
                    "--no-header" => {
                        header = false;
                        i += 1;
                    }
                    flag if flag.starts_with("--") => {
                        return Err(format!("unknown option: {flag}"));
                    }
                    value => {
                        sql_parts.push(value.to_string());
                        i += 1;
                    }
                }
            }
            if sql_parts.is_empty() {
                return Err("missing SQL".to_string());
            }
            let sql = sql_parts.join(" ");
            if cmd == "query" {
                Ok(Command::Query {
                    sql,
                    max_rows,
                    prefetch,
                    header,
                })
            } else {
                Ok(Command::Bench {
                    sql,
                    runs: runs.max(1),
                    prefetch,
                })
            }
        }
        other => Err(format!("unknown command: {other}\n\n{USAGE}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn query_with_options() {
        let cmd = parse(&args(&[
            "query",
            "select 1 from dual",
            "--max-rows",
            "50",
            "--prefetch",
            "2000",
            "--no-header",
        ]))
        .unwrap();
        assert_eq!(
            cmd,
            Command::Query {
                sql: "select 1 from dual".to_string(),
                max_rows: 50,
                prefetch: 2000,
                header: false,
            }
        );
    }

    #[test]
    fn sql_in_multiple_parts() {
        let cmd = parse(&args(&["query", "select", "1", "from", "dual"])).unwrap();
        match cmd {
            Command::Query { sql, .. } => assert_eq!(sql, "select 1 from dual"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn bench_runs() {
        let cmd = parse(&args(&["bench", "select 1 from dual", "--runs", "5"])).unwrap();
        assert_eq!(
            cmd,
            Command::Bench {
                sql: "select 1 from dual".to_string(),
                runs: 5,
                prefetch: 1000,
            }
        );
    }

    #[test]
    fn usage_errors() {
        assert!(parse(&[]).is_err());
        assert!(parse(&args(&["query"])).is_err());
        assert!(parse(&args(&["query", "select 1", "--runs", "x"])).is_err());
        assert!(parse(&args(&["query", "select 1", "--desconocida"])).is_err());
        assert!(parse(&args(&["diag", "extra"])).is_err());
        assert!(parse(&args(&["otro"])).is_err());
    }

    #[test]
    fn help() {
        assert_eq!(parse(&args(&["--help"])).unwrap(), Command::Help);
        assert_eq!(parse(&args(&["diag"])).unwrap(), Command::Diag);
    }
}
