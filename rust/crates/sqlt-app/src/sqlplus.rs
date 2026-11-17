// SPDX-License-Identifier: GPL-3.0-only
//! "Execute in SQL*Plus": the script is written to a temporary `.SQL` file and
//! the configured command line is launched, mirroring
//! `CPLSWorksheetDoc::OnSqlExecuteInSQLPlus` (`SQLWorksheetDoc3.cpp:54-197`).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

/// `SQLToolsSettings` defaults (`SQLToolsSettings.cpp:116-117`).
pub const DEFAULT_EXECUTABLE: &str = "$(SQLPLUS)";
pub const DEFAULT_COMMAND_LINE: &str =
    "$(USER)/$(PASSWORD)@$(CONNECT_STRING) $(SYS_PRIVS) @$(FILENAME)";

pub struct Fields<'a> {
    pub user: &'a str,
    pub password: &'a str,
    pub connect_string: &'a str,
    pub sys_privs: &'a str,
    pub filename: &'a str,
}

/// Replaces the `$(...)` placeholders the original knows. Unknown placeholders
/// are left untouched, like `Common::Substitutor`.
pub fn expand(template: &str, fields: &Fields) -> String {
    [
        ("$(SQLPLUS)", "sqlplus"),
        ("$(USER)", fields.user),
        ("$(PASSWORD)", fields.password),
        ("$(CONNECT_STRING)", fields.connect_string),
        ("$(SYS_PRIVS)", fields.sys_privs),
        ("$(FILENAME)", fields.filename),
    ]
    .iter()
    .fold(template.to_string(), |text, (placeholder, value)| {
        text.replace(placeholder, value)
    })
}

/// Executable and command line expanded together, as the original concatenates
/// them before launching.
pub fn expanded_command(executable: &str, command_line: &str, fields: &Fields) -> String {
    format!(
        "{} {}",
        expand(executable, fields),
        expand(command_line, fields)
    )
}

/// Writes the script with CRLF line endings and a trailing newline, which is
/// what the original writes line by line (`SQLWorksheetDoc3.cpp:137-154`).
pub fn write_script(dir: &Path, name: &str, text: &str) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let mut body = text.replace("\r\n", "\n").replace('\n', "\r\n");
    if !body.ends_with("\r\n") {
        body.push_str("\r\n");
    }
    let path = dir.join(name);
    fs::write(&path, body)?;
    Ok(path)
}

pub struct ParsedCommand {
    pub program: String,
    pub args: Vec<String>,
}

/// Splits the expanded command line the way `CreateProcess` does for the simple
/// case the templates use: whitespace separates arguments, double quotes group.
pub fn parse_command_line(line: &str) -> ParsedCommand {
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut has_token = false;
    for ch in line.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                has_token = true;
            }
            ch if ch.is_whitespace() && !quoted => {
                if has_token {
                    parts.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            ch => {
                current.push(ch);
                has_token = true;
            }
        }
    }
    if has_token {
        parts.push(current);
    }
    let mut parts = parts.into_iter();
    ParsedCommand {
        program: parts.next().unwrap_or_default(),
        args: parts.collect(),
    }
}

pub fn spawn(command: &ParsedCommand, dir: &Path) -> io::Result<Child> {
    Command::new(&command.program)
        .args(&command.args)
        .current_dir(dir)
        .spawn()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> Fields<'static> {
        Fields {
            user: "anamnesis",
            password: "secret#1",
            connect_string: "desa02",
            sys_privs: "",
            filename: "C:\\Temp\\sqlt 1.sql",
        }
    }

    #[test]
    fn expand_substitutes_every_placeholder() {
        let line = expanded_command(DEFAULT_EXECUTABLE, DEFAULT_COMMAND_LINE, &fields());
        assert_eq!(
            line,
            "sqlplus anamnesis/secret#1@desa02  @C:\\Temp\\sqlt 1.sql"
        );
    }

    #[test]
    fn expand_keeps_unknown_placeholders() {
        let fields = fields();
        assert_eq!(
            expand("$(SQLPLUS) $(UNKNOWN) $(USER)", &fields),
            "sqlplus $(UNKNOWN) anamnesis"
        );
        let mut sysdba = fields;
        sysdba.sys_privs = " AS SYSDBA";
        assert_eq!(
            expand("$(USER)$(SYS_PRIVS)@$(CONNECT_STRING)", &sysdba),
            "anamnesis AS SYSDBA@desa02"
        );
    }

    #[test]
    fn write_script_uses_crlf_and_trailing_newline() {
        let dir = std::env::temp_dir().join(format!("sqlt-sqlplus-test-{}", std::process::id()));
        let path = write_script(&dir, "script.sql", "select 1\nselect 2").expect("write");
        let body = fs::read_to_string(&path).expect("read");
        assert_eq!(body, "select 1\r\nselect 2\r\n");
        let again = write_script(&dir, "script.sql", "select 1\r\nselect 2\r\n").expect("rewrite");
        assert_eq!(fs::read_to_string(&again).expect("read"), body);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn parse_command_line_keeps_quoted_paths() {
        let parsed = parse_command_line("sqlplus anamnesis/secret@desa02 @\"C:\\tmp\\a b.sql\"");
        assert_eq!(parsed.program, "sqlplus");
        assert_eq!(
            parsed.args,
            vec![
                "anamnesis/secret@desa02".to_string(),
                "@C:\\tmp\\a b.sql".to_string()
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn expanded_command_runs_the_script() {
        use std::process::Command;
        let dir = std::env::temp_dir().join(format!("sqlt-sqlplus-run-{}", std::process::id()));
        let path = write_script(&dir, "script.sql", "select 1 from dual").expect("write");
        let filename = format!("\"{}\"", path.display());
        let fields = Fields {
            user: "u",
            password: "p",
            connect_string: "c",
            sys_privs: "",
            filename: &filename,
        };
        // `cmd /c type` proves the quoted $(FILENAME) reaches the child and the
        // file is written with CRLF line endings.
        let line = expanded_command("cmd", "/c type $(FILENAME)", &fields);
        let parsed = parse_command_line(&line);
        let output = Command::new(&parsed.program)
            .args(&parsed.args)
            .output()
            .expect("run");
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "select 1 from dual\r\n"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(windows)]
    #[test]
    fn spawn_runs_the_program() {
        let dir = std::env::temp_dir();
        let parsed = parse_command_line("cmd /c exit 0");
        let status = spawn(&parsed, &dir).expect("spawn").wait().expect("wait");
        assert!(status.success());
    }
}
