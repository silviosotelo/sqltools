// SPDX-License-Identifier: GPL-3.0-only
//! Bind variables (`:name`) and literal substitution, ported from
//! `ui/BindParser.cpp`. They are located outside text, comments and `q'[...]'`.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindKind {
    Auto,
    String,
    Number,
    Date,
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindValue {
    pub kind: BindKind,
    pub value: String,
}

impl Default for BindValue {
    fn default() -> Self {
        BindValue {
            kind: BindKind::Auto,
            value: String::new(),
        }
    }
}

struct Hit {
    start: usize,
    len: usize,
    name: String,
}

fn is_part(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '$' | '#')
}

fn scan(chars: &[char]) -> Vec<Hit> {
    let n = chars.len();
    let mut hits = Vec::new();
    let mut i = 0usize;
    while i < n {
        let c = chars[i];
        if c == '\'' {
            i += 1;
            while i < n {
                if chars[i] == '\'' {
                    if i + 1 < n && chars[i + 1] == '\'' {
                        i += 2;
                    } else {
                        i += 1;
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            continue;
        }
        if (c == 'q' || c == 'Q')
            && i + 2 < n
            && chars[i + 1] == '\''
            && (i == 0 || !is_part(chars[i - 1]))
        {
            let open = chars[i + 2];
            let close = match open {
                '[' => ']',
                '(' => ')',
                '{' => '}',
                '<' => '>',
                other => other,
            };
            let mut k = i + 3;
            let mut end = None;
            while k + 1 < n {
                if chars[k] == close && chars[k + 1] == '\'' {
                    end = Some(k + 2);
                    break;
                }
                k += 1;
            }
            i = end.unwrap_or(n);
            continue;
        }
        if c == '"' {
            i += 1;
            while i < n && chars[i] != '"' {
                i += 1;
            }
            if i < n {
                i += 1;
            }
            continue;
        }
        if c == '-' && i + 1 < n && chars[i + 1] == '-' {
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && i + 1 < n && chars[i + 1] == '*' {
            let mut k = i + 2;
            let mut end = None;
            while k + 1 < n {
                if chars[k] == '*' && chars[k + 1] == '/' {
                    end = Some(k + 2);
                    break;
                }
                k += 1;
            }
            i = end.unwrap_or(n);
            continue;
        }
        if c == ':'
            && i + 1 < n
            && (i == 0 || chars[i - 1] != ':')
            && (chars[i + 1].is_alphabetic() || chars[i + 1].is_numeric() || chars[i + 1] == '_')
        {
            let mut j = i + 1;
            while j < n && is_part(chars[j]) {
                j += 1;
            }
            let name: String = chars[i + 1..j].iter().collect();
            hits.push(Hit {
                start: i,
                len: j - i,
                name,
            });
            i = j;
            continue;
        }
        i += 1;
    }
    hits
}

/// Unique names in order of appearance, preserving original case.
pub fn names(sql: &str) -> Vec<String> {
    let chars: Vec<char> = sql.chars().collect();
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for hit in scan(&chars) {
        let key = hit.name.to_uppercase();
        if seen.insert(key) {
            out.push(hit.name);
        }
    }
    out
}

fn is_number(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0usize;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut has_digits = i > int_start;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let frac_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i > frac_start {
            has_digits = true;
        }
    }
    if !has_digits {
        return false;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let exp_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == exp_start {
            return false;
        }
    }
    i == b.len()
}

fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 && b.len() != 19 {
        return false;
    }
    let digits = |idx: &[usize]| idx.iter().all(|&i| b[i].is_ascii_digit());
    if b[4] != b'-' || b[7] != b'-' || !digits(&[0, 1, 2, 3, 5, 6, 8, 9]) {
        return false;
    }
    if b.len() == 19 {
        if b[10] != b' ' || b[13] != b':' || b[16] != b':' {
            return false;
        }
        if !digits(&[11, 12, 14, 15, 17, 18]) {
            return false;
        }
    }
    true
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// SQL del valor: `NULL`, `42`, `'texto'`, `to_date(...)`.
pub fn literal(v: &BindValue) -> (String, Option<String>) {
    let t = v.value.trim();
    if v.kind == BindKind::Null || (v.value.is_empty() && v.kind != BindKind::String) {
        return ("NULL".to_string(), None);
    }
    match v.kind {
        BindKind::Number => {
            if is_number(t) {
                (t.to_string(), None)
            } else {
                (
                    "NULL".to_string(),
                    Some(format!("{t} is not a number: using NULL.")),
                )
            }
        }
        BindKind::Date => {
            if is_date(t) {
                let lit = if t.len() > 10 {
                    format!("to_date({}, 'YYYY-MM-DD HH24:MI:SS')", quote(t))
                } else {
                    format!("to_date({}, 'YYYY-MM-DD')", quote(t))
                };
                (lit, None)
            } else {
                (
                    "NULL".to_string(),
                    Some(format!(
                        "{t} is not a date YYYY-MM-DD [HH:MM:SS]: using NULL."
                    )),
                )
            }
        }
        BindKind::String => {
            if v.value.is_empty() {
                ("NULL".to_string(), None)
            } else {
                (quote(&v.value), None)
            }
        }
        BindKind::Auto => {
            if is_number(t) {
                (t.to_string(), None)
            } else {
                (quote(&v.value), None)
            }
        }
        BindKind::Null => unreachable!(),
    }
}

/// Replaces from the end backwards so positions are not shifted.
pub fn substitute(sql: &str, values: &HashMap<String, BindValue>) -> (String, Vec<String>) {
    let chars: Vec<char> = sql.chars().collect();
    let hits = scan(&chars);
    let mut out = String::with_capacity(sql.len());
    let mut warnings = Vec::new();
    let mut last = 0usize;
    for hit in hits {
        out.extend(chars[last..hit.start].iter());
        let value = values
            .get(&hit.name.to_uppercase())
            .cloned()
            .unwrap_or_default();
        let (lit, warning) = literal(&value);
        if let Some(w) = warning {
            warnings.push(format!(":{} — {}", hit.name, w));
        }
        out.push_str(&lit);
        last = hit.start + hit.len;
    }
    out.extend(chars[last..].iter());
    (out, warnings)
}
