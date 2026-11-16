// SPDX-License-Identifier: GPL-3.0-only
//! Statement classification for read-only mode.

fn first_keyword(sql: &str) -> Option<String> {
    let chars: Vec<char> = sql.chars().collect();
    let n = chars.len();
    let mut i = 0usize;
    loop {
        while i < n && chars[i].is_whitespace() {
            i += 1;
        }
        if i + 1 < n && chars[i] == '-' && chars[i + 1] == '-' {
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if i + 1 < n && chars[i] == '/' && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < n && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(n);
            continue;
        }
        break;
    }
    if i >= n {
        return None;
    }
    let start = i;
    while i < n && (chars[i].is_alphanumeric() || chars[i] == '_') {
        i += 1;
    }
    Some(
        chars[start..i]
            .iter()
            .collect::<String>()
            .to_ascii_lowercase(),
    )
}

/// `true` si la sentencia no puede modificar datos ni esquema.
pub fn is_read_only(sql: &str) -> bool {
    match first_keyword(sql) {
        None => true,
        Some(k) => matches!(
            k.as_str(),
            "select" | "with" | "explain" | "describe" | "desc" | "show"
        ),
    }
}
