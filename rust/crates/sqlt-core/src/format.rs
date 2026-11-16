// SPDX-License-Identifier: GPL-3.0-only
//! Pragmatic SQL formatter. It preserves strings, q-quotes and comments,
//! uppercases keywords, and breaks major clauses onto their own lines.
//! The original C++ formatter is richer; this is the first P4 slice.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatOptions {
    pub uppercase_keywords: bool,
    pub indent: usize,
}

impl Default for FormatOptions {
    fn default() -> Self {
        FormatOptions {
            uppercase_keywords: true,
            indent: 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Word(String),
    Text(String),
    Symbol(char),
    Operator(String),
}

const KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "GROUP",
    "ORDER",
    "BY",
    "HAVING",
    "UNION",
    "ALL",
    "MINUS",
    "INTERSECT",
    "CONNECT",
    "START",
    "WITH",
    "INSERT",
    "INTO",
    "UPDATE",
    "DELETE",
    "VALUES",
    "SET",
    "RETURNING",
    "MERGE",
    "JOIN",
    "LEFT",
    "RIGHT",
    "INNER",
    "OUTER",
    "ON",
    "AND",
    "OR",
    "NOT",
    "NULL",
    "IS",
    "IN",
    "EXISTS",
    "BETWEEN",
    "LIKE",
    "AS",
    "DISTINCT",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "BEGIN",
    "DECLARE",
    "IF",
    "LOOP",
    "FOR",
    "WHILE",
    "CREATE",
    "ALTER",
    "DROP",
    "TABLE",
    "VIEW",
    "INDEX",
    "PACKAGE",
    "PROCEDURE",
    "FUNCTION",
    "TRIGGER",
    "TYPE",
    "COMMIT",
    "ROLLBACK",
    "EXPLAIN",
    "PLAN",
    "DESCRIBE",
];

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '$' | '#')
}

fn tokenize(input: &str) -> Vec<Token> {
    let chars: Vec<char> = input.chars().collect();
    let n = chars.len();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    while i < n {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if (c == 'q' || c == 'Q')
            && i + 2 < n
            && chars[i + 1] == '\''
            && (i == 0 || !is_word_char(chars[i - 1]))
        {
            let open = chars[i + 2];
            let close = match open {
                '[' => ']',
                '(' => ')',
                '{' => '}',
                '<' => '>',
                other => other,
            };
            let start = i;
            let mut j = i + 3;
            while j + 1 < n {
                if chars[j] == close && chars[j + 1] == '\'' {
                    j += 2;
                    break;
                }
                j += 1;
            }
            tokens.push(Token::Text(chars[start..j.min(n)].iter().collect()));
            i = j.min(n);
            continue;
        }
        if c == '\'' || c == '"' {
            let quote = c;
            let start = i;
            i += 1;
            while i < n {
                if chars[i] == quote {
                    if quote == '\'' && i + 1 < n && chars[i + 1] == '\'' {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            tokens.push(Token::Text(chars[start..i].iter().collect()));
            continue;
        }
        if c == '-' && i + 1 < n && chars[i + 1] == '-' {
            let start = i;
            while i < n && chars[i] != '\n' {
                i += 1;
            }
            tokens.push(Token::Text(chars[start..i].iter().collect()));
            continue;
        }
        if c == '/' && i + 1 < n && chars[i + 1] == '*' {
            let start = i;
            i += 2;
            while i + 1 < n && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(n);
            tokens.push(Token::Text(chars[start..i].iter().collect()));
            continue;
        }
        if is_word_char(c) {
            let start = i;
            while i < n && is_word_char(chars[i]) {
                i += 1;
            }
            tokens.push(Token::Word(chars[start..i].iter().collect()));
            continue;
        }
        let two: String = chars[i..(i + 2).min(n)].iter().collect();
        if matches!(two.as_str(), "<=" | ">=" | "<>" | "!=" | ":=" | "||" | "**") {
            tokens.push(Token::Operator(two));
            i += 2;
            continue;
        }
        if matches!(c, '(' | ')' | ',' | ';' | '.' | ':') {
            tokens.push(Token::Symbol(c));
            i += 1;
            continue;
        }
        tokens.push(Token::Operator(c.to_string()));
        i += 1;
    }
    tokens
}

fn clause_break(word: &str, next: Option<&str>) -> bool {
    match word {
        "SELECT" | "FROM" | "WHERE" | "HAVING" | "UNION" | "MINUS" | "INTERSECT" | "CONNECT"
        | "START" | "INSERT" | "UPDATE" | "DELETE" | "VALUES" | "SET" | "RETURNING" | "MERGE"
        | "WITH" => true,
        "GROUP" | "ORDER" => next == Some("BY"),
        _ => false,
    }
}

fn cmp_operator(op: &str) -> bool {
    matches!(
        op,
        "=" | "<" | ">" | "<=" | ">=" | "<>" | "!=" | ":=" | "||"
    )
}

pub fn format_sql(input: &str, options: &FormatOptions) -> String {
    let tokens = tokenize(input);
    let mut out = String::new();
    let mut depth = 0i32;
    let mut need_space = false;
    let mut line_start = true;
    let indent = " ".repeat(options.indent);

    let newline = |out: &mut String, line_start: &mut bool| {
        while out.ends_with(' ') {
            out.pop();
        }
        if !out.is_empty() {
            out.push('\n');
            *line_start = true;
        }
    };

    for (index, token) in tokens.iter().enumerate() {
        match token {
            Token::Word(word) => {
                let upper = word.to_ascii_uppercase();
                let next = tokens.get(index + 1).and_then(|next| match next {
                    Token::Word(value) => Some(value.to_ascii_uppercase()),
                    _ => None,
                });
                if depth == 0 && clause_break(&upper, next.as_deref()) {
                    newline(&mut out, &mut line_start);
                } else if depth == 0 && (upper == "AND" || upper == "OR") {
                    newline(&mut out, &mut line_start);
                    out.push_str(&indent);
                    line_start = false;
                    need_space = false;
                }
                if need_space && !line_start {
                    out.push(' ');
                }
                if options.uppercase_keywords && KEYWORDS.contains(&upper.as_str()) {
                    out.push_str(&upper);
                } else {
                    out.push_str(word);
                }
                need_space = true;
                line_start = false;
            }
            Token::Text(text) => {
                if need_space && !line_start {
                    out.push(' ');
                }
                out.push_str(text);
                need_space = true;
                line_start = false;
            }
            Token::Symbol('(') => {
                if need_space && !line_start {
                    out.push(' ');
                }
                out.push('(');
                depth += 1;
                need_space = false;
                line_start = false;
            }
            Token::Symbol(')') => {
                depth -= 1;
                out.push(')');
                need_space = true;
                line_start = false;
            }
            Token::Symbol(',') => {
                out.push(',');
                out.push(' ');
                need_space = false;
                line_start = false;
            }
            Token::Symbol(';') => {
                out.push(';');
                newline(&mut out, &mut line_start);
                need_space = false;
            }
            Token::Symbol('.') | Token::Symbol(':') => {
                out.push(match token {
                    Token::Symbol('.') => '.',
                    _ => ':',
                });
                need_space = false;
                line_start = false;
            }
            Token::Symbol(other) => {
                out.push(*other);
                need_space = false;
                line_start = false;
            }
            Token::Operator(op) => {
                if cmp_operator(op) {
                    if !out.ends_with(' ') && !line_start {
                        out.push(' ');
                    }
                    out.push_str(op);
                    out.push(' ');
                    need_space = false;
                } else {
                    if need_space && !line_start {
                        out.push(' ');
                    }
                    out.push_str(op);
                    need_space = true;
                }
                line_start = false;
            }
        }
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_a_simple_select() {
        let sql = "select a,b from t where x=1 and y=2 order by a";
        let formatted = format_sql(sql, &FormatOptions::default());
        assert_eq!(
            formatted,
            "SELECT a, b\nFROM t\nWHERE x = 1\n  AND y = 2\nORDER BY a"
        );
    }

    #[test]
    fn preserves_strings_and_comments() {
        let sql = "select 'a b', q'[x;y]' from t -- comment\n where x=1";
        let formatted = format_sql(sql, &FormatOptions::default());
        assert!(formatted.contains("'a b'"));
        assert!(formatted.contains("q'[x;y]'"));
        assert!(formatted.contains("-- comment"));
        assert!(formatted.contains("WHERE x = 1"));
    }

    #[test]
    fn is_idempotent_for_simple_sql() {
        let first = format_sql(
            "select a,b from t where x=1 and y=2",
            &FormatOptions::default(),
        );
        let second = format_sql(&first, &FormatOptions::default());
        assert_eq!(first, second);
    }
}
