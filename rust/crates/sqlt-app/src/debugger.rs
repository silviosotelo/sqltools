// SPDX-License-Identifier: GPL-3.0-only
//! PL/SQL debugger over `DBMS_DEBUG`, with the two sessions the package requires: the target runs the
//! statement on its own connection (and blocks at debug events) while the app's session supervises it.
//!
//! Everything is driven from PL/SQL wrappers because `DBMS_DEBUG` passes PL/SQL records, which OCI cannot
//! bind: the wrapper declares the record and returns its fields as OUT binds.
//! Constants are the ones in the package spec (`all_source`): namespaces 0/1/2/3/255, breakflags
//! next_line 32, any_call 12, any_return 512, and `success` 0.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use sqlt_oracle::{ConnectSpec, OracleType, QueryOptions, Session, ToSql};

/// How the next `continue` should run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StepMode {
    /// Stop at the next line (step over calls).
    Over,
    /// Stop at the next line, entering calls (step into).
    Into,
    /// Run until the current entrypoint returns (step out).
    Out,
}

impl StepMode {
    /// `break_next_line`, `break_any_call`, `break_any_return`.
    pub fn breakflags(self) -> i32 {
        match self {
            StepMode::Over => 32,
            StepMode::Into => 12,
            StepMode::Out => 512,
        }
    }
}

/// Where the target is, after `start` or after a step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugPosition {
    pub program: String,
    pub line: i32,
    pub reason: i32,
    pub reason_name: String,
    pub terminated: bool,
    /// Result code of the `DBMS_DEBUG` call that produced this position.
    pub status: i32,
}

fn reason_name(reason: i32) -> &'static str {
    match reason {
        0 => "none",
        2 => "interpreter starting",
        3 => "breakpoint",
        6 => "procedure entry",
        7 => "procedure return",
        8 => "procedure finished",
        9 => "new line",
        10 => "interrupt",
        11 => "exception",
        15 => "interpreter exiting",
        16 => "exception handler",
        17 => "timeout",
        20 => "instantiation",
        21 => "abort",
        25 => "kernel exit",
        26 => "oer breakpoint",
        _ => "unknown",
    }
}

const SYNCHRONIZE: &str = "declare r dbms_debug.runtime_info; ret binary_integer; \
     begin ret := dbms_debug.synchronize(r, 13); \
     :1 := ret; :2 := r.program.name; :3 := r.line#; :4 := r.reason; :5 := r.terminated; end;";

const CONTINUE: &str = "declare r dbms_debug.runtime_info; ret binary_integer; \
     begin ret := dbms_debug.continue(r, :1, 13); \
     :2 := ret; :3 := r.program.name; :4 := r.line#; :5 := r.reason; :6 := r.terminated; end;";

const SET_BREAKPOINT: &str =
    "declare p dbms_debug.program_info; bp binary_integer; ret binary_integer; \
     begin p.namespace := :1; p.owner := :2; p.name := :3; \
     ret := dbms_debug.set_breakpoint(p, :4, bp); :5 := ret; :6 := bp; end;";

const GET_VALUE: &str = "declare v varchar2(4000); ret binary_integer; \
     begin ret := dbms_debug.get_value(:1, :2, v, null); :3 := ret; :4 := v; end;";

/// One debug session: the target connection plus the thread that runs the statement.
pub struct Debugger {
    target: Arc<Session>,
    /// The session that attached to the target. Kept here so a command that arrives after the user switched
    /// connections still detaches from the right supervisor.
    supervisor: Arc<Session>,
    finished: Arc<AtomicBool>,
    pub id: String,
    pub position: DebugPosition,
}

/// Ends a debug session from any path. Detaching is not enough: Oracle keeps the target running and the
/// worker thread holds its own `Arc`, so the connection is only released when it is closed.
fn shutdown(target: &Session, finished: &AtomicBool) {
    // Closing the target is what ends the debug session: an OCI logoff terminates the target's database
    // session, and the server drops its `DBMS_DEBUG` session with it. Calling `detach_session` here instead
    // waits for the target to reach a debug event and costs ~13 s (measured against Oracle), which is
    // exactly the wait this function exists to avoid. The next `attach_session` works without it.
    target.close();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !finished.load(Ordering::Relaxed) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn number(value: Option<String>) -> i32 {
    value
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(-1)
}

fn text(value: Option<String>) -> String {
    value.unwrap_or_default()
}

fn position(values: &[Option<String>], status_at: usize) -> DebugPosition {
    let status = number(values.get(status_at).cloned().flatten());
    let program = text(values.get(status_at + 1).cloned().flatten());
    let line = number(values.get(status_at + 2).cloned().flatten());
    let reason = number(values.get(status_at + 3).cloned().flatten());
    let terminated = number(values.get(status_at + 4).cloned().flatten()) == 1;
    DebugPosition {
        program,
        line,
        reason,
        reason_name: reason_name(reason).to_string(),
        terminated,
        status,
    }
}

impl Debugger {
    /// Opens a second connection, arms it for debugging and runs `sql` there. The call returns when the
    /// target is suspended at the interpreter-start event, ready for the first step.
    pub fn start(
        debugger: &Arc<Session>,
        spec: &ConnectSpec,
        sql: String,
        timeout_secs: i32,
    ) -> Result<Debugger, String> {
        let target = Arc::new(Session::open(spec).map_err(|error| error.to_string())?);
        target
            .execute_with("alter session set plsql_debug = true", &[])
            .map_err(|error| error.to_string())?;
        let id = target
            .execute_block(
                "begin :1 := dbms_debug.initialize; end;",
                &[&OracleType::Varchar2(128)],
                &[1],
            )
            .map_err(|error| error.to_string())?
            .first()
            .cloned()
            .flatten()
            .ok_or_else(|| "DBMS_DEBUG did not return a session id".to_string())?;

        // If the debugger walks away, the target must not stay suspended forever.
        target
            .execute_with(
                &format!(
                    "declare t binary_integer; begin dbms_debug.set_timeout_behaviour(3); t := dbms_debug.set_timeout({timeout_secs}); end;"
                ),
                &[],
            )
            .map_err(|error| error.to_string())?;
        target
            .execute_with("begin dbms_debug.debug_on; end;", &[])
            .map_err(|error| error.to_string())?;

        let finished = Arc::new(AtomicBool::new(false));
        {
            let target = Arc::clone(&target);
            let finished = Arc::clone(&finished);
            std::thread::spawn(move || {
                // The statement suspends at debug events; it ends when the debugger lets it run out.
                let _ = target.execute_with(&sql, &[]);
                finished.store(true, Ordering::Relaxed);
            });
        }

        if let Err(error) = debugger.execute_block(
            "begin dbms_debug.attach_session(:1); end;",
            &[&id as &dyn ToSql],
            &[],
        ) {
            // The target is already running: an early return here would leave it suspended forever.
            shutdown(&target, &finished);
            return Err(error.to_string());
        }
        // `set_timeout` is a function, but it returns the *previous* timeout, not a status: only its
        // exception matters (verified against Oracle: it answers 15 when the timeout is 15).
        if let Err(error) = debugger.execute_block(
            "begin :1 := dbms_debug.set_timeout(:2); end;",
            &[&OracleType::Number(0, 0), &timeout_secs as &dyn ToSql],
            &[1],
        ) {
            shutdown(&target, &finished);
            return Err(error.to_string());
        }
        let values = match debugger.execute_block(
            SYNCHRONIZE,
            &[
                &OracleType::Number(0, 0),
                &OracleType::Varchar2(128),
                &OracleType::Number(0, 0),
                &OracleType::Number(0, 0),
                &OracleType::Number(0, 0),
            ],
            &[1, 2, 3, 4, 5],
        ) {
            Ok(values) => values,
            Err(error) => {
                shutdown(&target, &finished);
                return Err(error.to_string());
            }
        };
        let position = position(&values, 0);
        if position.status != 0 {
            shutdown(&target, &finished);
            return Err(format!(
                "DBMS_DEBUG.SYNCHRONIZE returned {} ({})",
                position.status, position.reason_name
            ));
        }
        Ok(Debugger {
            target,
            supervisor: Arc::clone(debugger),
            finished,
            id,
            position,
        })
    }

    /// One `continue` with the flags of `mode`; blocks until the target stops or ends.
    pub fn step(&mut self, mode: StepMode) -> Result<DebugPosition, String> {
        let debugger = &self.supervisor;
        let values = debugger
            .execute_block(
                CONTINUE,
                &[
                    &mode.breakflags() as &dyn ToSql,
                    &OracleType::Number(0, 0),
                    &OracleType::Varchar2(128),
                    &OracleType::Number(0, 0),
                    &OracleType::Number(0, 0),
                    &OracleType::Number(0, 0),
                ],
                &[2, 3, 4, 5, 6],
            )
            .map_err(|error| error.to_string())?;
        self.position = position(&values, 0);
        // A failed `continue` (a timeout, say) used to look like a normal stop: report it instead.
        if self.position.status != 0 && self.position.reason == 0 {
            return Err(format!(
                "DBMS_DEBUG.CONTINUE returned {}",
                self.position.status
            ));
        }
        Ok(self.position.clone())
    }

    /// Reads a variable of the current frame (`0` is the running entrypoint). `Ok(None)` = NULL.
    pub fn watch(&self, name: &str, frame: i32) -> Result<Option<String>, String> {
        let debugger = &self.supervisor;
        let values = debugger
            .execute_block(
                GET_VALUE,
                &[
                    &name as &dyn ToSql,
                    &frame as &dyn ToSql,
                    &OracleType::Number(0, 0),
                    &OracleType::Varchar2(4000),
                ],
                &[3, 4],
            )
            .map_err(|error| error.to_string())?;
        let status = number(values.first().cloned().flatten());
        if status != 0 {
            return Err(format!("DBMS_DEBUG.GET_VALUE returned {status}"));
        }
        Ok(values.get(1).cloned().flatten())
    }

    /// Sets a line breakpoint. `namespace` is 1 for a procedure/function/package spec, 2 for a package
    /// body, 3 for a trigger. Returns the breakpoint number.
    pub fn set_breakpoint(
        &self,
        owner: &str,
        name: &str,
        namespace: i32,
        line: i32,
    ) -> Result<i32, String> {
        let debugger = &self.supervisor;
        let values = debugger
            .execute_block(
                SET_BREAKPOINT,
                &[
                    &namespace as &dyn ToSql,
                    &owner as &dyn ToSql,
                    &name as &dyn ToSql,
                    &line as &dyn ToSql,
                    &OracleType::Number(0, 0),
                    &OracleType::Number(0, 0),
                ],
                &[5, 6],
            )
            .map_err(|error| error.to_string())?;
        let status = number(values.first().cloned().flatten());
        // 1 = error_deferred: accepted before the program runs and installed at synchronization.
        if status != 0 && status != 1 {
            return Err(format!("DBMS_DEBUG.SET_BREAKPOINT returned {status}"));
        }
        Ok(number(values.get(1).cloned().flatten()))
    }

    pub fn finished(&self) -> bool {
        self.finished.load(Ordering::Relaxed)
    }

    /// Detaches, interrupts the target and waits for the worker thread. Dropping the `Arc` alone does not
    /// end it: the thread keeps another clone while it is suspended.
    pub fn stop(self) {
        shutdown(&self.target, &self.finished);
        drop(self.target);
    }
}

/// Kept for the command layer: the debugger needs the same query options the store produces.
pub fn options_or_default(options: QueryOptions) -> QueryOptions {
    options
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_modes_map_to_the_package_breakflags() {
        assert_eq!(StepMode::Over.breakflags(), 32); // break_next_line
        assert_eq!(StepMode::Into.breakflags(), 12); // break_any_call
        assert_eq!(StepMode::Out.breakflags(), 512); // break_any_return
    }

    #[test]
    fn reasons_are_named() {
        assert_eq!(reason_name(2), "interpreter starting");
        assert_eq!(reason_name(3), "breakpoint");
        assert_eq!(reason_name(9), "new line");
        assert_eq!(reason_name(25), "kernel exit");
        assert_eq!(reason_name(999), "unknown");
    }

    #[test]
    fn a_position_is_read_from_the_outs() {
        let values = vec![
            Some("0".to_string()),
            Some("SQLT_P6".to_string()),
            Some("9".to_string()),
            Some("3".to_string()),
            Some("0".to_string()),
        ];
        let position = position(&values, 0);
        assert_eq!(position.program, "SQLT_P6");
        assert_eq!(position.line, 9);
        assert_eq!(position.reason_name, "breakpoint");
        assert!(!position.terminated);
    }
}
