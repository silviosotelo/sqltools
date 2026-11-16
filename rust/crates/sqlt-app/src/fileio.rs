// SPDX-License-Identifier: GPL-3.0-only
//! SQL file I/O that keeps the original line-ending style.

use std::fs;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileText {
    /// Text normalized to `\n` (CodeMirror's internal form).
    pub text: String,
    /// Original line ending: `\r\n` or `\n`.
    pub eol: String,
}

pub fn detect_eol(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else if text.contains('\r') {
        "\r"
    } else {
        "\n"
    }
}

/// Strips a UTF-8 BOM and normalizes CRLF/CR to `\n`, keeping the original EOL.
pub fn normalize_newlines(text: &str) -> (String, String) {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let eol = detect_eol(text).to_string();
    (text.replace("\r\n", "\n").replace('\r', "\n"), eol)
}

/// Converts `\n` back to the requested EOL (`\r\n`, `\r` or `\n`).
pub fn encode_newlines(text: &str, eol: &str) -> String {
    match eol {
        "\r\n" => text.replace('\n', "\r\n"),
        "\r" => text.replace('\n', "\r"),
        _ => text.to_string(),
    }
}

pub fn read_sql_file(path: &str) -> Result<FileText, String> {
    let raw = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let (text, eol) = normalize_newlines(&raw);
    Ok(FileText { text, eol })
}

pub fn write_sql_file(path: &str, text: &str, eol: &str) -> Result<(), String> {
    fs::write(path, encode_newlines(text, eol)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_and_normalizes_line_endings() {
        let (text, eol) = normalize_newlines("a\r\nb\rc\n");
        assert_eq!(text, "a\nb\nc\n");
        assert_eq!(eol, "\r\n");
        let (text, eol) = normalize_newlines("\u{feff}a\nb");
        assert_eq!(text, "a\nb");
        assert_eq!(eol, "\n");
        let (text, eol) = normalize_newlines("a\rb\r");
        assert_eq!(text, "a\nb\n");
        assert_eq!(eol, "\r");
    }

    #[test]
    fn encodes_the_requested_line_ending() {
        assert_eq!(encode_newlines("a\nb", "\r\n"), "a\r\nb");
        assert_eq!(encode_newlines("a\nb", "\n"), "a\nb");
        assert_eq!(encode_newlines("a\nb", "\r"), "a\rb");
    }

    #[test]
    fn file_roundtrip_keeps_crlf() {
        let path = std::env::temp_dir().join(format!("sqlt-fileio-{}.sql", std::process::id()));
        let path_text = path.display().to_string();
        write_sql_file(&path_text, "select 1\nfrom dual\n", "\r\n").unwrap();
        let raw = fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\r\nfrom dual\r\n"), "{raw:?}");
        let file = read_sql_file(&path_text).unwrap();
        assert_eq!(file.text, "select 1\nfrom dual\n");
        assert_eq!(file.eol, "\r\n");
        let _ = fs::remove_file(path);
    }
}
