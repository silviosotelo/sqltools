// SPDX-License-Identifier: GPL-3.0-only
//! SQL/PL-SQL script splitter, ported from the C++/Qt port's `ui/SqlSplitter.cpp`.
//!
//! Rules:
//! - Plain SQL ends at `;` outside text and comments.
//! - An anonymous block (`BEGIN`/`DECLARE`) ends at the `;` closing its last
//!   `END` (BEGIN/IF/LOOP/CASE are counted against their END).
//! - `CREATE [OR REPLACE] PROCEDURE/FUNCTION/PACKAGE/TRIGGER/TYPE` and similar
//!   end only at a line containing only `/`.
//! - A line containing only `/` always closes the current statement.
//!
//! Positions are UTF-16 units so they match CodeMirror and the original
//! `QString` from the original port.

/// Una sentencia del script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    /// UTF-16 offset of the first useful character.
    pub start: usize,
    /// UTF-16 offset of the end of `text` (SQL excludes `;`; PL/SQL
    /// keeps the final `END` `;`, same as the C++ port).
    pub end: usize,
    /// PL/SQL block (keeps the final `END` `;` when executing).
    pub plsql: bool,
    /// Line (1-based) where it starts.
    pub start_line: usize,
    /// Text ready to execute.
    pub text: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Sql,
    Anonymous,
    Create,
}

/// Palabras clave que, tras `CREATE [OR REPLACE] [EDITIONABLE|NONEDITIONABLE]`,
/// empiezan un objeto con cuerpo PL/SQL.
fn is_plsql_create(words: &[String]) -> bool {
    let mut i = 1; // words[0] == "create"
    if i < words.len() && words[i] == "or" {
        i += 2; // or replace
    }
    if i < words.len() && (words[i] == "editionable" || words[i] == "noneditionable") {
        i += 1;
    }
    if i >= words.len() {
        return false;
    }
    matches!(
        words[i].as_str(),
        "procedure" | "function" | "package" | "trigger" | "type" | "library" | "java"
    )
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$' || c == '#'
}

/// Divide un script en sentencias.
pub fn split(script: &str) -> Vec<Statement> {
    let chars: Vec<char> = script.chars().collect();
    let n = chars.len();

    // UTF-16 offset of each character index (and the end).
    let mut utf16 = Vec::with_capacity(n + 1);
    let mut acc = 0usize;
    for &c in &chars {
        utf16.push(acc);
        acc += c.len_utf16();
    }
    utf16.push(acc);

    let newlines: Vec<usize> = chars
        .iter()
        .enumerate()
        .filter(|(_, c)| **c == '\n')
        .map(|(i, _)| i)
        .collect();
    let line_of = |pos: usize| newlines.partition_point(|&p| p < pos) + 1;

    // If a line containing only '/' starts at `pos`, return that line's end.
    let is_slash_line = |pos: usize| -> Option<usize> {
        let mut line_start = pos;
        while line_start > 0 && chars[line_start - 1] != '\n' {
            line_start -= 1;
        }
        if !chars[line_start..pos].iter().all(|c| c.is_whitespace()) {
            return None;
        }
        if chars.get(pos) != Some(&'/') {
            return None;
        }
        let mut line_end = pos + 1;
        while line_end < n && chars[line_end] != '\n' {
            line_end += 1;
        }
        if chars[pos + 1..line_end].iter().all(|c| c.is_whitespace()) {
            Some(line_end)
        } else {
            None
        }
    };

    let find_seq = |from: usize, a: char, b: char| -> Option<usize> {
        let mut k = from;
        while k + 1 < n {
            if chars[k] == a && chars[k + 1] == b {
                return Some(k);
            }
            k += 1;
        }
        None
    };

    let mut out = Vec::new();
    let mut i = 0usize;

    while i < n {
        // Saltar blancos, comentarios entre sentencias, y ';' o '/' sueltos.
        loop {
            if i >= n {
                break;
            }
            let c = chars[i];
            if c.is_whitespace() || c == ';' {
                i += 1;
                continue;
            }
            if c == '-' && chars.get(i + 1) == Some(&'-') {
                while i < n && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            if c == '/' && chars.get(i + 1) == Some(&'*') {
                i = match find_seq(i + 2, '*', '/') {
                    Some(e) => e + 2,
                    None => n,
                };
                continue;
            }
            if let Some(e) = is_slash_line(i) {
                i = e;
                continue;
            }
            break;
        }
        if i >= n {
            break;
        }

        let start = i;
        let mut kind = Kind::Sql;
        let mut first_words: Vec<String> = Vec::new();
        let mut classified = false;
        let mut depth: i32 = 0;
        let mut saw_begin = false;
        let mut stmt_end: Option<usize> = None;
        let mut next = n;

        let mut j = i;
        while j < n {
            let c = chars[j];

            if c == '\'' {
                // Texto '...' con '' como comilla escapada.
                j += 1;
                while j < n {
                    if chars[j] == '\'' {
                        if j + 1 < n && chars[j + 1] == '\'' {
                            j += 2;
                        } else {
                            j += 1;
                            break;
                        }
                    } else {
                        j += 1;
                    }
                }
                continue;
            }
            if c == '"' {
                j += 1;
                while j < n && chars[j] != '"' {
                    j += 1;
                }
                if j < n {
                    j += 1;
                }
                continue;
            }
            if (c == 'q' || c == 'Q')
                && j + 2 < n
                && chars[j + 1] == '\''
                && (j == 0 || !is_word_char(chars[j - 1]))
            {
                // Literales alternativos q'[ ... ]', q'{ ... }', q'( ... )', q'< ... >', q'X ... X'.
                let open = chars[j + 2];
                let close = match open {
                    '[' => ']',
                    '(' => ')',
                    '{' => '}',
                    '<' => '>',
                    other => other,
                };
                let mut k = j + 3;
                let mut end = None;
                while k + 1 < n {
                    if chars[k] == close && chars[k + 1] == '\'' {
                        end = Some(k + 2);
                        break;
                    }
                    k += 1;
                }
                j = end.unwrap_or(n);
                continue;
            }
            if c == '-' && chars.get(j + 1) == Some(&'-') {
                while j < n && chars[j] != '\n' {
                    j += 1;
                }
                continue;
            }
            if c == '/' && chars.get(j + 1) == Some(&'*') {
                j = match find_seq(j + 2, '*', '/') {
                    Some(e) => e + 2,
                    None => n,
                };
                continue;
            }

            if c == '/' {
                // Line containing only '/'.
                if let Some(e) = is_slash_line(j) {
                    stmt_end = Some(j);
                    next = e;
                    break;
                }
            }

            if is_word_char(c) && (j == 0 || !is_word_char(chars[j - 1])) {
                let mut k = j;
                while k < n && is_word_char(chars[k]) {
                    k += 1;
                }
                let word: String = chars[j..k].iter().collect::<String>().to_lowercase();

                if !classified {
                    first_words.push(word.clone());
                    if first_words[0] == "begin" || first_words[0] == "declare" {
                        kind = Kind::Anonymous;
                        classified = true;
                    } else if first_words[0] == "create" {
                        let second = first_words.get(1).map(String::as_str);
                        let classify = first_words.len() >= 5
                            || (first_words.len() >= 2
                                && second != Some("or")
                                && second != Some("editionable")
                                && second != Some("noneditionable"))
                            || (first_words.len() >= 4 && second == Some("or"));
                        if classify {
                            kind = if is_plsql_create(&first_words) {
                                Kind::Create
                            } else {
                                Kind::Sql
                            };
                            classified = true;
                        }
                    } else {
                        classified = true;
                    }
                }

                if kind == Kind::Anonymous {
                    if word == "begin" {
                        depth += 1;
                        saw_begin = true;
                    } else if word == "if" || word == "loop" || word == "case" {
                        depth += 1;
                    } else if word == "end" {
                        // "end if", "end loop", "end case" cierran su propia estructura;
                        // A bare "end" closes BEGIN or a CASE expression.
                        let mut m = k;
                        while m < n && chars[m].is_whitespace() {
                            m += 1;
                        }
                        let mut m2 = m;
                        while m2 < n && is_word_char(chars[m2]) {
                            m2 += 1;
                        }
                        let nx: String = chars[m..m2].iter().collect::<String>().to_lowercase();
                        if nx == "if" || nx == "loop" || nx == "case" {
                            k = m2;
                        }
                        depth -= 1;
                    }
                }

                j = k;
                continue;
            }

            if c == ';' {
                if kind == Kind::Sql || (kind == Kind::Anonymous && saw_begin && depth <= 0) {
                    stmt_end = Some(if kind == Kind::Sql { j } else { j + 1 });
                    next = j + 1;
                    break;
                }
                if kind == Kind::Create {
                    j += 1;
                    continue;
                }
            }

            j += 1;
        }

        let end = stmt_end.unwrap_or(n);
        let mut trimmed_end = end;
        while trimmed_end > start && chars[trimmed_end - 1].is_whitespace() {
            trimmed_end -= 1;
        }
        let text: String = chars[start..trimmed_end].iter().collect();
        if !text.trim().is_empty() {
            out.push(Statement {
                start: utf16[start],
                end: utf16[trimmed_end],
                plsql: kind != Kind::Sql,
                start_line: line_of(start),
                text,
            });
        }
        i = next;
    }

    out
}

/// Index of the statement containing the position (or the previous one if it falls in
/// whitespace between two); `None` if there is none.
pub fn statement_at(stmts: &[Statement], position: usize) -> Option<usize> {
    let mut best = None;
    for (k, st) in stmts.iter().enumerate() {
        if position >= st.start && position <= st.end {
            return Some(k);
        }
        if st.end < position {
            best = Some(k);
        }
    }
    best
}
