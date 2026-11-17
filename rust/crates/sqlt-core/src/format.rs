// SPDX-License-Identifier: GPL-3.0-only
//! SQL formatter: clause layout, one item per line, JOIN/ON, AND/OR, operator
//! spacing and comma position. Golden cases come from `tests/format_test.cpp`.
//!
//! PL/SQL blocks are re-indented by block/IF/LOOP/EXCEPTION context; embedded
//! SQL is formatted with the clause rules.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum FormatStyle {
    Standard,
    River,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum KeywordCase {
    Upper,
    Lower,
    Keep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FormatOptions {
    /// Spaces (or tabs) per indentation level.
    pub indent: usize,
    pub use_tabs: bool,
    pub keyword_case: KeywordCase,
    /// Case of function names (a word directly followed by `(`).
    pub function_case: KeywordCase,
    /// Case of non-keyword identifiers.
    pub identifier_case: KeywordCase,
    pub style: FormatStyle,
    /// Put the comma at the start of the line instead of the end.
    pub comma_leading: bool,
    /// Indentation of AND/OR; `None` derives it from the style.
    pub and_or_indent: Option<usize>,
    /// If set, list items start on the keyword line and align at this column.
    pub cont_indent: Option<usize>,
    /// Indentation of JOIN lines; `None` derives it from the style.
    pub join_indent: Option<usize>,
    /// Put the ON clause of a join on its own line.
    pub on_new_line: bool,
    /// Space after `,` in SQL expressions.
    pub space_after_comma: bool,
    /// Space just inside `(` and `)`.
    pub space_in_parens: bool,
    /// Break AND/OR onto their own line (`condLines(..., breaks)`).
    pub and_or_new_line: bool,
    /// One item per line for the SELECT/GROUP BY/ORDER BY/SET lists.
    pub one_item_per_line: bool,
    /// Standard style: keep the keyword alone when a list has several items.
    pub kw_alone_when_many: bool,
    /// Width at which a comma-separated parenthesised group breaks one item per
    /// line (`Fmt::inlineText`).
    pub max_line: usize,
    pub align_declarations: bool,
    pub align_assignments: bool,
    pub blank_after_declare: bool,
    pub blank_before_exception: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        FormatOptions {
            indent: 2,
            use_tabs: false,
            keyword_case: KeywordCase::Upper,
            function_case: KeywordCase::Upper,
            identifier_case: KeywordCase::Keep,
            style: FormatStyle::Standard,
            comma_leading: false,
            and_or_indent: None,
            cont_indent: None,
            join_indent: None,
            on_new_line: false,
            space_after_comma: true,
            space_in_parens: false,
            and_or_new_line: true,
            one_item_per_line: true,
            kw_alone_when_many: true,
            max_line: 100,
            align_declarations: false,
            align_assignments: false,
            blank_after_declare: false,
            blank_before_exception: false,
        }
    }
}

/// Infers a profile from a sample. Mirrors `FormatOptions::inferFromExample`
/// (`ui/SqlFormatter.cpp:630-675`): the clause columns give the continuation,
/// AND/OR, JOIN and ON layout; reserved-word casing decides the keyword case;
/// the most frequent indent step fixes the indent; comma/parenthesis spacing and
/// aligned declarations come from text patterns.
pub fn infer_from_example(sql: &str) -> FormatOptions {
    let mut options = FormatOptions::default();
    let mut indent_deltas: std::collections::BTreeMap<usize, usize> =
        std::collections::BTreeMap::new();
    let mut previous_indent: Option<usize> = None;
    let mut select_col: Option<usize> = None;
    let mut select_alone = false;
    let mut item_col: Option<usize> = None;
    let mut comma_inline = false;
    let mut await_item_line = false;
    let mut from_col: Option<usize> = None;
    let mut where_col: Option<usize> = None;
    let mut and_col: Option<usize> = None;
    let mut join_col: Option<usize> = None;
    let mut on_own_line = false;
    let mut leading_comma = false;
    let mut aligned_declarations = 0usize;

    for line in sql.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        // `indentOf`: spaces count one column, tabs four (`SqlFormatter.cpp:633`).
        let leading = leading_columns(line);
        if line.starts_with('\t') {
            options.use_tabs = true;
        }
        // `indentOf` deltas between consecutive levels (`SqlFormatter.cpp:636`).
        if let Some(previous) = previous_indent {
            if leading > previous && leading - previous <= 8 {
                *indent_deltas.entry(leading - previous).or_insert(0) += 1;
            }
        }
        previous_indent = Some(leading);
        if trimmed.starts_with(',') {
            leading_comma = true;
        }
        if is_aligned_declaration(line) {
            aligned_declarations += 1;
        }
        let first = trimmed
            .split(|c: char| c.is_whitespace() || c == '(')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        // The SELECT-alone lookahead does not consume the line (the original
        // walks forward from the SELECT line without skipping it).
        if await_item_line {
            if item_col.is_none() {
                item_col = Some(leading);
            }
            await_item_line = false;
        }
        if first == "SELECT" && select_col.is_none() {
            select_col = Some(leading);
            let rest = trimmed.get(6..).map(str::trim_start).unwrap_or("");
            select_alone = rest.is_empty();
            comma_inline = !rest.is_empty() && rest.contains(',') && !rest.ends_with(',');
            await_item_line = select_alone;
        } else if first == "FROM" && from_col.is_none() && select_col.is_some() {
            from_col = Some(leading);
        } else if first == "WHERE" && where_col.is_none() {
            where_col = Some(leading);
        } else if matches!(first.as_str(), "AND" | "OR") && and_col.is_none() && where_col.is_some()
        {
            and_col = Some(leading);
        } else if matches!(first.as_str(), "JOIN" | "LEFT" | "INNER" | "RIGHT" | "FULL")
            && join_col.is_none()
            && from_col.is_some()
        {
            join_col = Some(leading);
        } else if first == "ON" && join_col.is_some() {
            on_own_line = true;
        }
    }

    // `SqlFormatter.cpp:643-645`: count reserved words by casing; ties are Upper.
    let mut upper_words = 0usize;
    let mut lower_words = 0usize;
    for token in tokenize(sql) {
        if let Token::Word(word) = token {
            let upper = word.to_ascii_uppercase();
            if KEYWORDS.contains(&upper.as_str()) {
                if word == upper {
                    upper_words += 1;
                } else if word == word.to_ascii_lowercase() {
                    lower_words += 1;
                }
            }
        }
    }
    if upper_words + lower_words > 0 {
        options.keyword_case = if upper_words >= lower_words {
            KeywordCase::Upper
        } else {
            KeywordCase::Lower
        };
    }
    // The most frequent step wins; `BTreeMap` iterates ascending and the
    // comparison is strict, so the smallest step wins a tie like the original.
    let mut best_indent = 0usize;
    let mut best_count = 0usize;
    for (step, count) in &indent_deltas {
        if *count > best_count {
            best_count = *count;
            best_indent = *step;
        }
    }
    if best_indent > 0 {
        options.indent = best_indent;
    }
    if let (Some(select_col), Some(from_col)) = (select_col, from_col) {
        // `SqlFormatter.cpp:656`: FROM further right than SELECT is River.
        options.style = if from_col > select_col {
            FormatStyle::River
        } else {
            FormatStyle::Standard
        };
    }
    if let (Some(where_col), Some(and_col)) = (where_col, and_col) {
        options.and_or_indent = Some(and_col.saturating_sub(where_col));
    }
    if let (Some(from_col), Some(join_col)) = (from_col, join_col) {
        options.join_indent = Some(join_col.saturating_sub(from_col));
    }
    if let Some(col_select) = select_col {
        match (select_alone, item_col) {
            (true, Some(col_item)) => {
                options.cont_indent = Some(col_item.saturating_sub(col_select));
                options.kw_alone_when_many = true;
            }
            (false, _) => options.kw_alone_when_many = false,
            _ => {}
        }
    }
    if comma_inline {
        options.one_item_per_line = false;
    }
    options.comma_leading = leading_comma;
    options.on_new_line = on_own_line;
    options.space_after_comma = !tight_comma(sql);
    options.space_in_parens = spaced_inside_parens(sql);
    options.align_declarations = aligned_declarations >= 2;
    options
}

/// `indentOf` (`SqlFormatter.cpp:633`): leading spaces count one column, tabs four.
fn leading_columns(line: &str) -> usize {
    let mut columns = 0;
    for ch in line.chars() {
        match ch {
            ' ' => columns += 1,
            '\t' => columns += 4,
            _ => break,
        }
    }
    columns
}

/// `,[A-Za-z0-9_']`: a comma with no space after it.
fn tight_comma(sample: &str) -> bool {
    let chars: Vec<char> = sample.chars().collect();
    chars.windows(2).any(|pair| {
        pair[0] == ',' && (pair[1].is_ascii_alphanumeric() || pair[1] == '_' || pair[1] == '\'')
    })
}

/// `\(\s[Z]` present while `\([Z]` is not.
fn spaced_inside_parens(sample: &str) -> bool {
    let chars: Vec<char> = sample.chars().collect();
    let opened = |ch: char| ch.is_ascii_alphanumeric() || ch == '_' || ch == '\'';
    let spaced = chars
        .windows(3)
        .any(|window| window[0] == '(' && window[1].is_whitespace() && opened(window[2]));
    let tight = chars
        .windows(2)
        .any(|window| window[0] == '(' && opened(window[1]));
    spaced && !tight
}

/// `^\s+[A-Za-z_][\w$#]*\s{2,}[A-Za-z_][^;]*;\s*$`.
fn is_aligned_declaration(line: &str) -> bool {
    let Some(body) = line.trim_end().strip_suffix(';') else {
        return false;
    };
    let rest = body.trim_start();
    if rest.len() == body.len() {
        return false;
    }
    let name_len = rest
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$' | '#'))
        .count();
    if name_len == 0 || !rest.starts_with(|ch: char| ch.is_ascii_alphabetic() || ch == '_') {
        return false;
    }
    let after = &rest[name_len..];
    let spaces = after.chars().take_while(|ch| *ch == ' ').count();
    if spaces < 2 {
        return false;
    }
    let tail = &after[spaces..];
    tail.starts_with(|ch: char| ch.is_ascii_alphabetic() || ch == '_') && !tail.contains(';')
}

fn indent_unit(options: &FormatOptions) -> String {
    if options.use_tabs {
        "\t".to_string()
    } else {
        " ".repeat(options.indent)
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
    "ACCESSIBLE",
    "ADD",
    "AFTER",
    "ALL",
    "ALTER",
    "AND",
    "ANY",
    "APPLY",
    "AS",
    "ASC",
    "AUTHID",
    "BEFORE",
    "BEGIN",
    "BETWEEN",
    "BINARY_INTEGER",
    "BLOB",
    "BODY",
    "BOOLEAN",
    "BULK",
    "BY",
    "CASE",
    "CHAR",
    "CHECK",
    "CLOB",
    "CLOSE",
    "COLLECT",
    "COLUMN",
    "COMMIT",
    "CONNECT",
    "CONSTANT",
    "CONSTRAINT",
    "CONTINUE",
    "CREATE",
    "CROSS",
    "CURRENT",
    "CURSOR",
    "CYCLE",
    "DATE",
    "DAY",
    "DECLARE",
    "DEFAULT",
    "DEFERRED",
    "DEFINER",
    "DELETE",
    "DELETING",
    "DENSE_RANK",
    "DESC",
    "DETERMINISTIC",
    "DISABLE",
    "DISTINCT",
    "DROP",
    "DUP_VAL_ON_INDEX",
    "EACH",
    "ELSE",
    "ELSIF",
    "ENABLE",
    "END",
    "EXCEPTION",
    "EXCLUSIVE",
    "EXECUTE",
    "EXISTS",
    "EXIT",
    "EXTENDS",
    "FALSE",
    "FETCH",
    "FINAL",
    "FIRST",
    "FOLLOWING",
    "FOR",
    "FORALL",
    "FORCE",
    "FOREIGN",
    "FROM",
    "FULL",
    "FUNCTION",
    "GLOBAL",
    "GOTO",
    "GRANT",
    "GROUP",
    "HAVING",
    "HOUR",
    "IF",
    "IMMEDIATE",
    "IN",
    "INDEX",
    "INITIALLY",
    "INNER",
    "INSERT",
    "INSERTING",
    "INSTANTIABLE",
    "INTERSECT",
    "INTERVAL",
    "INTO",
    "INVALID_NUMBER",
    "IS",
    "ISOLATION",
    "JOIN",
    "KEEP",
    "KEY",
    "LAG",
    "LAST",
    "LATERAL",
    "LEAD",
    "LEFT",
    "LEVEL",
    "LIKE",
    "LIMIT",
    "LOCKED",
    "LOOP",
    "MATCHED",
    "MEMBER",
    "MERGE",
    "MINUS",
    "MINUTE",
    "MODEL",
    "MODIFY",
    "MONTH",
    "NATURAL",
    "NESTED",
    "NEW",
    "NEXT",
    "NO_DATA_FOUND",
    "NOCYCLE",
    "NOT",
    "NOVALIDATE",
    "NOWAIT",
    "NULL",
    "NULLS",
    "NUMBER",
    "OBJECT",
    "OF",
    "OFFSET",
    "OLD",
    "ON",
    "ONLY",
    "OPEN",
    "OR",
    "ORDER",
    "OTHERS",
    "OUT",
    "OUTER",
    "OVER",
    "OVERRIDING",
    "PACKAGE",
    "PARALLEL_ENABLE",
    "PARTITION",
    "PIPELINED",
    "PIVOT",
    "PLS_INTEGER",
    "PRAGMA",
    "PRECEDING",
    "PRESERVE",
    "PRIMARY",
    "PRIOR",
    "PROCEDURE",
    "RAISE",
    "RANGE",
    "RANK",
    "RECORD",
    "REF",
    "REFERENCES",
    "REFERENCING",
    "REPLACE",
    "RESULT_CACHE",
    "RETURN",
    "RETURNING",
    "REVOKE",
    "RIGHT",
    "ROLLBACK",
    "ROW",
    "ROWS",
    "ROWTYPE",
    "SAVEPOINT",
    "SECOND",
    "SELECT",
    "SELF",
    "SEQUENCE",
    "SET",
    "SIBLINGS",
    "SKIP",
    "SOME",
    "START",
    "STATIC",
    "SUBTYPE",
    "TABLE",
    "TEMPORARY",
    "THEN",
    "TIMESTAMP",
    "TO",
    "TOO_MANY_ROWS",
    "TRANSACTION",
    "TRIGGER",
    "TRUE",
    "TRUNCATE",
    "TYPE",
    "UNBOUNDED",
    "UNDER",
    "UNION",
    "UNIQUE",
    "UNPIVOT",
    "UPDATE",
    "UPDATING",
    "USING",
    "VALIDATE",
    "VALUE_ERROR",
    "VALUES",
    "VARCHAR",
    "VARCHAR2",
    "VARRAY",
    "VIEW",
    "WAIT",
    "WHEN",
    "WHERE",
    "WHILE",
    "WITH",
    "WITHIN",
    "WORK",
    "YEAR",
    "ZERO_DIVIDE",
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

fn next_word(tokens: &[Token], index: usize) -> Option<String> {
    tokens.get(index + 1).and_then(|token| match token {
        Token::Word(word) => Some(word.to_ascii_uppercase()),
        _ => None,
    })
}

fn detect_clause(tokens: &[Token], index: usize) -> Option<(String, usize)> {
    let Token::Word(word) = tokens.get(index)? else {
        return None;
    };
    let upper = word.to_ascii_uppercase();
    match upper.as_str() {
        "GROUP" | "ORDER" if next_word(tokens, index).as_deref() == Some("BY") => {
            Some((format!("{upper} BY"), 2))
        }
        "INSERT" if next_word(tokens, index).as_deref() == Some("INTO") => {
            Some(("INSERT INTO".to_string(), 2))
        }
        "DELETE" if next_word(tokens, index).as_deref() == Some("FROM") => {
            Some(("DELETE FROM".to_string(), 2))
        }
        "SELECT" | "FROM" | "WHERE" | "HAVING" | "SET" | "VALUES" | "UPDATE" | "MERGE" | "INTO" => {
            Some((upper, 1))
        }
        _ => None,
    }
}

fn should_space_before_paren(previous: Option<&Token>) -> bool {
    matches!(
        previous,
        Some(Token::Word(word))
            if matches!(
                word.to_ascii_uppercase().as_str(),
                "IN" | "EXISTS" | "VALUES" | "INTO" | "FROM" | "WHERE" | "AND" | "OR" | "ON" | "SET"
            )
    )
}

fn render(tokens: &[Token], options: &FormatOptions) -> String {
    render_at(tokens, options, 0)
}

/// `render` with the column the expression starts at; `maxLine` wrapping needs it
/// (`Fmt::inlineText(items, col)` in the original).
fn render_at(tokens: &[Token], options: &FormatOptions, col: usize) -> String {
    let mut out = String::new();
    let mut tight = false;
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        if let Some((piece, next)) = wrapped_group(tokens, index, options, col, &out) {
            if should_space_before_paren(tokens.get(index.wrapping_sub(1))) {
                out.push(' ');
            }
            out.push_str(&piece);
            tight = false;
            index = next;
            continue;
        }
        match token {
            Token::Word(word) => {
                if !tight {
                    if let Some(last) = out.chars().last() {
                        if !matches!(last, ' ' | '\n' | '(' | '.' | ':') {
                            out.push(' ');
                        }
                    }
                }
                tight = false;
                let upper = word.to_ascii_uppercase();
                let is_keyword = KEYWORDS.contains(&upper.as_str());
                let is_function = matches!(tokens.get(index + 1), Some(Token::Symbol('(')));
                let case = if is_keyword {
                    options.keyword_case
                } else if is_function {
                    options.function_case
                } else {
                    options.identifier_case
                };
                match case {
                    KeywordCase::Upper => out.push_str(&upper),
                    KeywordCase::Lower => out.push_str(&word.to_lowercase()),
                    KeywordCase::Keep => out.push_str(word),
                }
            }
            Token::Text(text) => {
                if !tight {
                    if let Some(last) = out.chars().last() {
                        if !matches!(last, ' ' | '\n' | '(' | '.') {
                            out.push(' ');
                        }
                    }
                }
                tight = false;
                out.push_str(text);
                // A line comment ends the line: the SQL that follows must not be
                // swallowed by it (`SqlFormatter.cpp:264`).
                if text.starts_with("--") {
                    out.push('\n');
                    tight = true;
                }
            }
            Token::Symbol('(') => {
                if should_space_before_paren(tokens.get(index.wrapping_sub(1))) {
                    out.push(' ');
                }
                out.push('(');
                if options.space_in_parens {
                    out.push(' ');
                }
            }
            Token::Symbol(')') => {
                if options.space_in_parens && !out.ends_with(' ') {
                    out.push(' ');
                }
                out.push(')');
            }
            Token::Symbol(',') => {
                out.push(',');
                if options.space_after_comma {
                    out.push(' ');
                }
                tight = !options.space_after_comma;
            }
            Token::Symbol(';') => out.push(';'),
            Token::Symbol('.') => out.push('.'),
            Token::Symbol(':') => out.push(':'),
            Token::Symbol(other) => out.push(*other),
            Token::Operator(op) => {
                let next_is_paren = matches!(tokens.get(index + 1), Some(Token::Symbol(')')));
                let prev_is_open =
                    matches!(tokens.get(index.wrapping_sub(1)), Some(Token::Symbol('(')));
                let unary = matches!(op.as_str(), "-" | "+")
                    && (out.is_empty()
                        || out.ends_with('(')
                        || out.ends_with(',')
                        || out.ends_with(' '));
                if unary {
                    out.push_str(op);
                    tight = true;
                } else if op == "*" && (next_is_paren || prev_is_open || out.ends_with('.')) {
                    out.push('*');
                } else if matches!(
                    op.as_str(),
                    "=" | "<" | ">" | "<=" | ">=" | "<>" | "!=" | ":=" | "||"
                ) {
                    if !out.ends_with(' ') {
                        out.push(' ');
                    }
                    out.push_str(op);
                    out.push(' ');
                } else {
                    if !out.ends_with(' ') && !out.is_empty() {
                        out.push(' ');
                    }
                    out.push_str(op);
                    out.push(' ');
                }
            }
        }
        index += 1;
    }
    out.trim().to_string()
}

/// `Fmt::inlineText`'s long group rule: a parenthesised, comma-separated group
/// that would overflow `maxLine` goes one item per line, indented one level from
/// `col`, with the closing parenthesis back at `col`.
fn wrapped_group(
    tokens: &[Token],
    open: usize,
    options: &FormatOptions,
    col: usize,
    out: &str,
) -> Option<(String, usize)> {
    if tokens.get(open) != Some(&Token::Symbol('(')) {
        return None;
    }
    let close = matching_paren(tokens, open)?;
    let inner = &tokens[open + 1..close];
    if is_subquery(inner) {
        // `Fmt::inlineText`: a subquery is always laid out as
        // `(\n` + query(inner, col + indent) + `\n` + indent(col) + `)`.
        let formatted = format_statement(inner, options);
        let pad = format!("{}{}", " ".repeat(col), indent_unit(options));
        let indented = formatted
            .lines()
            .map(|line| format!("{pad}{line}"))
            .collect::<Vec<_>>()
            .join("\n");
        let piece = format!("(\n{indented}\n{})", " ".repeat(col));
        return Some((piece, close + 1));
    }
    let parts = split_commas(inner);
    if parts.len() < 2 {
        return None;
    }
    let flat: Vec<String> = parts
        .iter()
        .map(|part| render_at(part, options, col))
        .collect();
    let one_line = format!(
        "({}{}{})",
        if options.space_in_parens { " " } else { "" },
        flat.join(if options.space_after_comma { ", " } else { "," }),
        if options.space_in_parens { " " } else { "" }
    );
    if one_line.contains('\n') {
        return None;
    }
    let last_len = match out.rfind('\n') {
        Some(position) => out.len() - position - 1,
        None => col + out.len(),
    };
    if last_len + one_line.len() <= options.max_line {
        return None;
    }
    let inner_indent = format!("{}{}", " ".repeat(col), indent_unit(options));
    let closing_indent = " ".repeat(col);
    let mut piece = String::from("(\n");
    for (index, item) in flat.iter().enumerate() {
        piece.push_str(&inner_indent);
        piece.push_str(item);
        if index + 1 < flat.len() {
            piece.push(',');
        }
        piece.push('\n');
    }
    piece.push_str(&closing_indent);
    piece.push(')');
    Some((piece, close + 1))
}

fn matching_paren(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        match token {
            Token::Symbol('(') => depth += 1,
            Token::Symbol(')') => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn is_subquery(tokens: &[Token]) -> bool {
    matches!(
        tokens.first(),
        Some(Token::Word(word))
            if word.eq_ignore_ascii_case("select") || word.eq_ignore_ascii_case("with")
    )
}

fn split_commas(tokens: &[Token]) -> Vec<Vec<Token>> {
    let mut parts = Vec::new();
    let mut current = Vec::new();
    let mut depth = 0i32;
    for token in tokens {
        match token {
            Token::Symbol('(') => depth += 1,
            Token::Symbol(')') => depth -= 1,
            Token::Symbol(',') if depth == 0 => {
                parts.push(current);
                current = Vec::new();
                continue;
            }
            _ => {}
        }
        current.push(token.clone());
    }
    parts.push(current);
    parts.retain(|part| !part.is_empty());
    parts
}

fn clause_text(keyword: &str, options: &FormatOptions) -> String {
    match options.keyword_case {
        KeywordCase::Lower => keyword.to_lowercase(),
        _ => keyword.to_string(),
    }
}

fn river_prefix(keyword: &str, options: &FormatOptions) -> String {
    let text = clause_text(keyword, options);
    match keyword {
        "SELECT" => format!("{text} "),
        "GROUP BY" | "ORDER BY" => format!(" {text} "),
        "SET" => format!("  {text} "),
        _ => format!("{text} "),
    }
}

/// Column where the clause keyword starts: the padding a River prefix carries
/// (`selectBase + pad` in the original). Standard prefixes have none, so 0.
fn clause_col(prefix: &str) -> usize {
    prefix.len() - prefix.trim_start().len()
}

fn format_list(keyword: &str, body: &[Token], options: &FormatOptions) -> String {
    let items = split_commas(body);
    let river = options.style == FormatStyle::River;
    let prefix = if river {
        river_prefix(keyword, options)
    } else {
        format!("{} ", clause_text(keyword, options))
    };
    if items.len() <= 1 {
        return format!("{prefix}{}", render(body, options));
    }
    let indent = indent_unit(options);
    // `query()` keeps the keyword alone only in Standard style, with several
    // items, when `oneItemPerLine` and `kwAloneWhenMany` are both on.
    let alone = !river && options.one_item_per_line && options.kw_alone_when_many;
    let cont = match options.cont_indent {
        Some(width) => " ".repeat(width),
        None if river => " ".repeat(prefix.len()),
        None => indent,
    };
    let comma = |index: usize| -> &'static str {
        if !options.comma_leading && index + 1 < items.len() {
            ","
        } else {
            ""
        }
    };
    let mut out = String::new();
    if alone {
        out.push_str(&clause_text(keyword, options));
        for (index, item) in items.iter().enumerate() {
            out.push('\n');
            out.push_str(&cont);
            if options.comma_leading && index > 0 {
                out.push_str(", ");
            }
            out.push_str(&render_at(item, options, cont.len()));
            out.push_str(comma(index));
        }
        return out;
    }
    out.push_str(&prefix);
    out.push_str(&render_at(&items[0], options, clause_col(&prefix)));
    out.push_str(comma(0));
    for (index, item) in items.iter().enumerate().skip(1) {
        out.push('\n');
        if options.comma_leading {
            // `indentStr(qMax(0, ec - 2))` then `", "`.
            out.push_str(&" ".repeat(cont.len().saturating_sub(2)));
            out.push_str(", ");
        } else {
            out.push_str(&cont);
        }
        out.push_str(&render_at(item, options, cont.len()));
        out.push_str(comma(index));
    }
    out
}

/// Splits a boolean expression at top-level AND/OR. The AND of `BETWEEN a AND b`
/// stays with the first part.
fn split_and_or(body: &[Token]) -> Vec<Vec<Token>> {
    let mut parts: Vec<Vec<Token>> = Vec::new();
    let mut current = Vec::new();
    let mut depth = 0i32;
    let mut between = false;
    for token in body {
        match token {
            Token::Symbol('(') => depth += 1,
            Token::Symbol(')') => depth -= 1,
            Token::Word(word) if depth == 0 => {
                let upper = word.to_ascii_uppercase();
                if upper == "BETWEEN" {
                    between = true;
                } else if upper == "AND" || upper == "OR" {
                    if between {
                        between = false;
                        current.push(token.clone());
                        continue;
                    }
                    parts.push(current);
                    current = vec![token.clone()];
                    continue;
                }
            }
            _ => {}
        }
        current.push(token.clone());
    }
    parts.push(current);
    parts
}

fn format_where(body: &[Token], options: &FormatOptions) -> String {
    let indent = indent_unit(options);
    let parts = if options.and_or_new_line {
        split_and_or(body)
    } else {
        vec![body.to_vec()]
    };
    let river = options.style == FormatStyle::River;
    let prefix = if river {
        format!(" {} ", clause_text("WHERE", options))
    } else {
        format!("{} ", clause_text("WHERE", options))
    };
    let and_indent = match options.and_or_indent {
        Some(width) => " ".repeat(width),
        None if river => "   ".to_string(),
        None => indent.clone(),
    };
    let col = clause_col(&prefix);
    let mut out = format!("{prefix}{}", render_at(&parts[0], options, col));
    for part in parts.iter().skip(1) {
        out.push_str(&format!("\n{and_indent}{}", render_at(part, options, col)));
    }
    out
}

fn format_from(body: &[Token], options: &FormatOptions) -> String {
    let indent = indent_unit(options);
    let river = options.style == FormatStyle::River;
    let prefix = if river {
        format!("  {} ", clause_text("FROM", options))
    } else {
        format!("{} ", clause_text("FROM", options))
    };
    if body.first() == Some(&Token::Symbol('('))
        && body.last() == Some(&Token::Symbol(')'))
        && body.len() >= 2
    {
        let inner = &body[1..body.len() - 1];
        let starts_select = inner
            .iter()
            .any(|token| matches!(token, Token::Word(word) if word.eq_ignore_ascii_case("select")));
        if starts_select {
            let formatted = format_statement(inner, options);
            let indented = formatted
                .lines()
                .map(|line| format!("{indent}{line}"))
                .collect::<Vec<_>>()
                .join("\n");
            return format!("{prefix}(\n{indented}\n)");
        }
    }
    let mut joins: Vec<Vec<Token>> = Vec::new();
    let mut current = Vec::new();
    let mut depth = 0i32;
    for (index, token) in body.iter().enumerate() {
        if depth == 0 {
            if let Token::Word(word) = token {
                let upper = word.to_ascii_uppercase();
                let modifier = matches!(
                    upper.as_str(),
                    "LEFT" | "RIGHT" | "FULL" | "INNER" | "CROSS"
                );
                let plain_join = upper == "JOIN"
                    && !current.iter().any(|item| {
                        matches!(
                            item,
                            Token::Word(inner)
                                if matches!(
                                    inner.to_ascii_uppercase().as_str(),
                                    "LEFT" | "RIGHT" | "FULL" | "INNER" | "CROSS"
                                )
                        )
                    });
                if (modifier || plain_join) && !current.is_empty() {
                    joins.push(current);
                    current = Vec::new();
                }
            }
        }
        match token {
            Token::Symbol('(') => depth += 1,
            Token::Symbol(')') => depth -= 1,
            _ => {}
        }
        current.push(token.clone());
        let _ = index;
    }
    if !current.is_empty() {
        joins.push(current);
    }
    if joins.is_empty() {
        return format!("{prefix}{}", render(body, options));
    }
    let join_indent = match options.join_indent {
        Some(width) => " ".repeat(width),
        None if river => "  ".to_string(),
        None => indent.clone(),
    };
    let mut out = format!(
        "{prefix}{}",
        render_at(&joins[0], options, clause_col(&prefix))
    );
    for join in joins.iter().skip(1) {
        let on_position = if options.on_new_line {
            let mut depth = 0i32;
            join.iter().position(|token| {
                match token {
                    Token::Symbol('(') => depth += 1,
                    Token::Symbol(')') => depth -= 1,
                    _ => {}
                }
                depth == 0 && matches!(token, Token::Word(word) if word.eq_ignore_ascii_case("on"))
            })
        } else {
            None
        };
        let Some(position) = on_position else {
            out.push_str(&format!(
                "\n{join_indent}{}",
                render_at(join, options, join_indent.len())
            ));
            continue;
        };
        out.push_str(&format!(
            "\n{join_indent}{}",
            render_at(&join[..position], options, join_indent.len())
        ));
        let on_indent = format!("{join_indent}{}", indent_unit(options));
        let parts = if options.and_or_new_line {
            split_and_or(&join[position + 1..])
        } else {
            vec![join[position + 1..].to_vec()]
        };
        out.push_str(&format!(
            "\n{on_indent}{} {}",
            clause_text("ON", options),
            render_at(&parts[0], options, on_indent.len())
        ));
        let cond_indent = match options.and_or_indent {
            Some(width) => format!("{on_indent}{}", " ".repeat(width)),
            None => format!("{on_indent}{}", indent_unit(options)),
        };
        for part in parts.iter().skip(1) {
            out.push_str(&format!(
                "\n{cond_indent}{}",
                render_at(part, options, cond_indent.len())
            ));
        }
    }
    out
}

fn format_statement(tokens: &[Token], options: &FormatOptions) -> String {
    let mut clauses: Vec<(String, Vec<Token>)> = Vec::new();
    let mut keyword: Option<String> = None;
    let mut body: Vec<Token> = Vec::new();
    let mut depth = 0i32;
    let mut index = 0usize;
    while index < tokens.len() {
        if depth == 0 {
            if let Some((clause, len)) = detect_clause(tokens, index) {
                if let Some(previous) = keyword.take() {
                    clauses.push((previous, body.clone()));
                    body.clear();
                }
                keyword = Some(clause);
                index += len;
                continue;
            }
        }
        if tokens[index] == Token::Symbol('(') {
            depth += 1;
        } else if tokens[index] == Token::Symbol(')') {
            depth -= 1;
        }
        body.push(tokens[index].clone());
        index += 1;
    }
    if let Some(clause) = keyword {
        clauses.push((clause, body));
    } else {
        return render(tokens, options);
    }

    let mut out = Vec::new();
    for (clause, body) in clauses {
        let text = match clause.as_str() {
            "SELECT" => format_list("SELECT", &body, options),
            "GROUP BY" | "ORDER BY" => format_list(&clause, &body, options),
            "FROM" => format_from(&body, options),
            "WHERE" => format_where(&body, options),
            "SET" => format_list("SET", &body, options),
            "VALUES" => format!(
                "{} {}",
                clause_text("VALUES", options),
                render(&body, options)
            ),
            "INSERT INTO" => {
                let mut body_options = *options;
                body_options.keyword_case = KeywordCase::Keep;
                body_options.function_case = KeywordCase::Keep;
                body_options.identifier_case = KeywordCase::Keep;
                let rendered = render(&body, &body_options);
                let with_space = match rendered.find('(') {
                    Some(position) => {
                        format!("{} {}", &rendered[..position], &rendered[position..])
                    }
                    None => rendered,
                };
                format!("{} {with_space}", clause_text("INSERT INTO", options))
            }
            _ => format!(
                "{} {}",
                clause_text(&clause, options),
                render(&body, options)
            ),
        };
        out.push(text);
    }
    out.join("\n")
}

fn split_statements(tokens: &[Token]) -> Vec<Vec<Token>> {
    let mut statements = Vec::new();
    let mut current = Vec::new();
    let mut depth = 0i32;
    for token in tokens {
        match token {
            Token::Symbol('(') => depth += 1,
            Token::Symbol(')') => depth -= 1,
            Token::Symbol(';') if depth == 0 => {
                statements.push(current);
                current = Vec::new();
                continue;
            }
            _ => {}
        }
        current.push(token.clone());
    }
    if !current.is_empty() {
        statements.push(current);
    }
    statements
}

fn is_plsql(tokens: &[Token]) -> bool {
    let first = tokens.iter().find_map(|token| match token {
        Token::Word(word) => Some(word.to_ascii_uppercase()),
        _ => None,
    });
    matches!(first.as_deref(), Some("BEGIN") | Some("DECLARE"))
        || (first.as_deref() == Some("CREATE")
            && tokens.iter().any(|token| {
                matches!(
                    token,
                    Token::Word(word)
                        if matches!(
                            word.to_ascii_uppercase().as_str(),
                            "PROCEDURE" | "FUNCTION" | "PACKAGE" | "TRIGGER" | "TYPE"
                        )
                )
            }))
}

/// PL/SQL block context. `open` is the indentation level the opening keyword
/// was printed at; the body lives at `open + 1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlCtx {
    Declare(usize),
    Block(usize),
    If(usize),
    Loop(usize),
    Case(usize),
    When(usize),
    Exception(usize),
}

impl PlCtx {
    fn open(self) -> usize {
        match self {
            PlCtx::Declare(open)
            | PlCtx::Block(open)
            | PlCtx::If(open)
            | PlCtx::Loop(open)
            | PlCtx::Case(open)
            | PlCtx::When(open)
            | PlCtx::Exception(open) => open,
        }
    }
}

fn body_level(stack: &[PlCtx]) -> usize {
    stack.last().map(|ctx| ctx.open() + 1).unwrap_or(0)
}

fn pop_until<F: Fn(PlCtx) -> bool>(stack: &mut Vec<PlCtx>, predicate: F) -> Option<PlCtx> {
    while let Some(ctx) = stack.pop() {
        if predicate(ctx) {
            return Some(ctx);
        }
    }
    None
}

fn uppercase_keyword(word: &str) -> String {
    let upper = word.to_ascii_uppercase();
    if KEYWORDS.contains(&upper.as_str()) {
        upper
    } else {
        word.to_string()
    }
}

/// Renders a PL/SQL control/plain line. The original only reformats embedded
/// SQL; other statements keep their punctuation spacing (e.g. `f(-1,'x')`).
fn render_plsql(tokens: &[Token]) -> String {
    let mut out = String::new();
    let mut tight = true;
    for (index, token) in tokens.iter().enumerate() {
        match token {
            Token::Word(word) => {
                if !tight {
                    out.push(' ');
                }
                tight = false;
                out.push_str(&uppercase_keyword(word));
            }
            Token::Text(text) => {
                if !tight {
                    out.push(' ');
                }
                tight = false;
                out.push_str(text);
            }
            Token::Symbol(c) => {
                if *c == '(' {
                    if should_space_before_paren(tokens.get(index.wrapping_sub(1))) {
                        out.push(' ');
                    }
                    out.push('(');
                } else {
                    out.push(*c);
                }
                tight = *c != ')';
            }
            Token::Operator(op) => {
                let unary = matches!(op.as_str(), "-" | "+")
                    && (tight || out.ends_with('(') || out.ends_with(','));
                if unary || matches!(op.as_str(), "%" | ".") {
                    out.push_str(op);
                    tight = true;
                } else {
                    if !tight && !out.is_empty() {
                        out.push(' ');
                    }
                    out.push_str(op);
                    out.push(' ');
                    tight = true;
                }
            }
        }
    }
    out.trim_end().to_string()
}

fn token_words(tokens: &[Token]) -> (Option<String>, Option<String>) {
    let mut words = tokens.iter().filter_map(|token| match token {
        Token::Word(word) => Some(word.to_ascii_uppercase()),
        _ => None,
    });
    (words.next(), words.next())
}

fn has_top_level_semicolon(text: &str) -> bool {
    let mut depth = 0i32;
    for token in tokenize(text) {
        match token {
            Token::Symbol('(') => depth += 1,
            Token::Symbol(')') => depth -= 1,
            Token::Symbol(';') if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

fn sql_starters(words: &str) -> bool {
    matches!(
        words,
        "SELECT" | "INSERT" | "UPDATE" | "DELETE" | "MERGE" | "WITH"
    )
}

fn header_ends_with_is(tokens: &[Token]) -> bool {
    let mut depth = 0i32;
    let mut found = false;
    for token in tokens {
        match token {
            Token::Symbol('(') => depth += 1,
            Token::Symbol(')') => depth -= 1,
            Token::Word(word) if depth == 0 => {
                let upper = word.to_ascii_uppercase();
                found = upper == "IS" || upper == "AS";
            }
            _ => {}
        }
    }
    found
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlLineKind {
    Decl,
    Stmt,
    Other,
}

struct PlLine {
    level: usize,
    text: String,
    kind: PlLineKind,
}

/// Aligns a run of `name rest` lines at the widest name (declarations).
fn align_declaration_run(run: &mut [PlLine]) {
    let mut parts: Vec<(String, String)> = Vec::new();
    let mut width = 0usize;
    for line in run.iter() {
        let split = line
            .text
            .find(char::is_whitespace)
            .unwrap_or(line.text.len());
        let name = line.text[..split].to_string();
        let rest = line.text[split..].trim_start().to_string();
        width = width.max(name.chars().count());
        parts.push((name, rest));
    }
    for (line, (name, rest)) in run.iter_mut().zip(parts) {
        let pad = width + 1 - name.chars().count();
        line.text = format!("{name}{}{rest}", " ".repeat(pad));
    }
}

/// Aligns `name := value;` assignments at the widest name.
fn align_assignment_run(run: &mut [PlLine]) {
    let mut parts: Vec<(String, String)> = Vec::new();
    let mut width = 0usize;
    for line in run.iter() {
        let Some(position) = line.text.find(" := ") else {
            parts.push((line.text.clone(), String::new()));
            continue;
        };
        let name = line.text[..position].to_string();
        let rest = line.text[position + 4..].to_string();
        width = width.max(name.chars().count());
        parts.push((name, rest));
    }
    for (line, (name, rest)) in run.iter_mut().zip(parts) {
        if rest.is_empty() {
            continue;
        }
        let pad = width + 1 - name.chars().count();
        line.text = format!("{name}{}:= {rest}", " ".repeat(pad));
    }
}

fn align_plsql_runs(lines: &mut [PlLine], options: &FormatOptions) {
    let mut index = 0usize;
    while index < lines.len() {
        let kind = lines[index].kind;
        let level = lines[index].level;
        let mut end = index;
        while end < lines.len() && lines[end].kind == kind && lines[end].level == level {
            end += 1;
        }
        if end - index > 1 {
            match kind {
                PlLineKind::Decl if options.align_declarations => {
                    align_declaration_run(&mut lines[index..end]);
                }
                PlLineKind::Stmt if options.align_assignments => {
                    align_assignment_run(&mut lines[index..end]);
                }
                _ => {}
            }
        }
        index = end;
    }
}

/// PL/SQL re-indentation. `ponytail:` line-oriented; blocks written as one
/// physical line (`begin null; end;`) keep their trailing text on that line.
fn format_plsql(input: &str, options: &FormatOptions) -> String {
    let indent = indent_unit(options);
    let raw: Vec<&str> = input.lines().collect();
    let mut stack: Vec<PlCtx> = Vec::new();
    let mut out: Vec<PlLine> = Vec::new();
    let mut index = 0usize;

    let emit = |out: &mut Vec<PlLine>, level: usize, text: &str, kind: PlLineKind| {
        let trimmed = text.trim_end();
        if trimmed.is_empty() {
            if out.last().is_some_and(|line| !line.text.is_empty()) {
                out.push(PlLine {
                    level: 0,
                    text: String::new(),
                    kind: PlLineKind::Other,
                });
            }
            return;
        }
        out.push(PlLine {
            level,
            text: trimmed.to_string(),
            kind,
        });
    };
    let blank = |out: &mut Vec<PlLine>| {
        if out.last().is_some_and(|line| !line.text.is_empty()) {
            out.push(PlLine {
                level: 0,
                text: String::new(),
                kind: PlLineKind::Other,
            });
        }
    };

    while index < raw.len() {
        let line = raw[index].trim();
        index += 1;
        if line.is_empty() {
            continue;
        }
        let tokens = tokenize(line);
        let (first, second) = token_words(&tokens);
        let first = first.unwrap_or_default();

        if sql_starters(&first) {
            let mut buffer = line.to_string();
            while !has_top_level_semicolon(&buffer) && index < raw.len() {
                buffer.push(' ');
                buffer.push_str(raw[index].trim());
                index += 1;
            }
            let mut sql = tokenize(&buffer);
            let semicolon = sql.last() == Some(&Token::Symbol(';'));
            if semicolon {
                sql.pop();
            }
            let mut text = format_statement(&sql, options);
            if semicolon {
                text.push(';');
            }
            let level = body_level(&stack);
            for chunk in text.lines() {
                emit(&mut out, level, chunk, PlLineKind::Other);
            }
            continue;
        }

        match first.as_str() {
            "CREATE" => {
                emit(
                    &mut out,
                    body_level(&stack),
                    &render_plsql(&tokens),
                    PlLineKind::Other,
                );
                if header_ends_with_is(&tokens) {
                    let open = body_level(&stack);
                    stack.push(PlCtx::Declare(open));
                }
            }
            "DECLARE" => {
                let open = body_level(&stack);
                emit(&mut out, open, &render_plsql(&tokens), PlLineKind::Other);
                stack.push(PlCtx::Declare(open));
            }
            "BEGIN" => {
                if matches!(stack.last(), Some(PlCtx::Declare(_))) {
                    stack.pop();
                    if options.blank_after_declare {
                        blank(&mut out);
                    }
                }
                let open = body_level(&stack);
                emit(&mut out, open, &render_plsql(&tokens), PlLineKind::Other);
                stack.push(PlCtx::Block(open));
            }
            "IF" => {
                let open = body_level(&stack);
                emit(&mut out, open, &render_plsql(&tokens), PlLineKind::Other);
                stack.push(PlCtx::If(open));
            }
            "ELSIF" | "ELSE" => {
                let open = stack
                    .iter()
                    .rev()
                    .find_map(|ctx| match ctx {
                        PlCtx::If(open) => Some(*open),
                        _ => None,
                    })
                    .unwrap_or_else(|| body_level(&stack));
                emit(&mut out, open, &render_plsql(&tokens), PlLineKind::Other);
            }
            "FOR" | "WHILE" | "LOOP" => {
                let open = body_level(&stack);
                emit(&mut out, open, &render_plsql(&tokens), PlLineKind::Other);
                stack.push(PlCtx::Loop(open));
            }
            "EXCEPTION" => {
                let open = match stack.iter().rev().find_map(|ctx| match ctx {
                    PlCtx::Block(open) => Some(*open),
                    _ => None,
                }) {
                    Some(open) => open,
                    None => body_level(&stack),
                };
                if options.blank_before_exception {
                    blank(&mut out);
                }
                emit(&mut out, open, &render_plsql(&tokens), PlLineKind::Other);
                stack.push(PlCtx::Exception(open));
            }
            "CASE" => {
                let open = body_level(&stack);
                emit(&mut out, open, &render_plsql(&tokens), PlLineKind::Other);
                stack.push(PlCtx::Case(open));
            }
            "WHEN" => {
                let open = body_level(&stack);
                emit(&mut out, open, &render_plsql(&tokens), PlLineKind::Other);
                stack.push(PlCtx::When(open));
            }
            "END" => {
                let text = render_plsql(&tokens);
                match second.as_deref() {
                    Some("IF") => {
                        let open = pop_until(&mut stack, |ctx| matches!(ctx, PlCtx::If(_)))
                            .map(PlCtx::open)
                            .unwrap_or_else(|| body_level(&stack));
                        emit(&mut out, open, &text, PlLineKind::Other);
                    }
                    Some("LOOP") => {
                        let open = pop_until(&mut stack, |ctx| matches!(ctx, PlCtx::Loop(_)))
                            .map(PlCtx::open)
                            .unwrap_or_else(|| body_level(&stack));
                        emit(&mut out, open, &text, PlLineKind::Other);
                    }
                    Some("CASE") => {
                        let open = pop_until(&mut stack, |ctx| matches!(ctx, PlCtx::Case(_)))
                            .map(PlCtx::open)
                            .unwrap_or_else(|| body_level(&stack));
                        emit(&mut out, open, &text, PlLineKind::Other);
                    }
                    _ => {
                        let open = pop_until(&mut stack, |ctx| matches!(ctx, PlCtx::Block(_)))
                            .map(PlCtx::open)
                            .unwrap_or_else(|| body_level(&stack));
                        emit(&mut out, open, &text, PlLineKind::Other);
                    }
                }
            }
            "/" => {
                stack.clear();
                emit(&mut out, 0, "/", PlLineKind::Other);
            }
            _ => {
                let kind = if matches!(stack.last(), Some(PlCtx::Declare(_))) {
                    PlLineKind::Decl
                } else {
                    PlLineKind::Stmt
                };
                emit(&mut out, body_level(&stack), &render_plsql(&tokens), kind);
            }
        }
    }

    align_plsql_runs(&mut out, options);
    let mut text = out
        .iter()
        .map(|line| {
            if line.text.is_empty() {
                String::new()
            } else {
                format!("{}{}", indent.repeat(line.level), line.text)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    text.push('\n');
    text
}

pub fn format_sql(input: &str, options: &FormatOptions) -> String {
    let tokens = tokenize(input);
    if tokens.is_empty() {
        return String::new();
    }
    if is_plsql(&tokens) {
        return format_plsql(input, options);
    }
    let had_semicolon = input.trim_end().ends_with(';');
    let statements = split_statements(&tokens);
    let formatted: Vec<String> = statements
        .iter()
        .map(|statement| format_statement(statement, options))
        .filter(|statement| !statement.is_empty())
        .collect();
    let mut out = formatted.join(";\n");
    if had_semicolon && !out.is_empty() {
        out.push(';');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn standard() -> FormatOptions {
        FormatOptions::default()
    }

    #[test]
    fn formats_the_cpp_standard_select() {
        let sql = "select a.id,b.nombre,sum(c.total) from cab_pedido a join cliente b on a.id_cliente=b.id left join det_pedido c on c.id_pedido=a.id where a.estado='A' and b.activo=1 group by a.id,b.nombre order by 3 desc;";
        let expected = "SELECT\n  a.id,\n  b.nombre,\n  SUM(c.total)\nFROM cab_pedido a\n  JOIN cliente b ON a.id_cliente = b.id\n  LEFT JOIN det_pedido c ON c.id_pedido = a.id\nWHERE a.estado = 'A'\n  AND b.activo = 1\nGROUP BY\n  a.id,\n  b.nombre\nORDER BY 3 DESC;";
        assert_eq!(format_sql(sql, &standard()), expected);
    }

    #[test]
    fn plsql_matches_cpp() {
        let sql = "create or replace procedure p_demo(p_id number) is\nv_id number;\nv_estado cab.estado%type;\nbegin\nif p_id is null then\nRAISE_APPLICATION_ERROR(-20999,'Debe declarar');\nend if;\nbegin\nselect estado into v_estado from cab where id_pedido=p_id and id_empresa=v_id;\nexception\nwhen no_data_found then\nRAISE_APPLICATION_ERROR(-20999,'No existe');\nend;\nfor r in (select 1 from dual) loop\nnull;\nend loop;\nend;\n/\n";
        let expected = "CREATE OR REPLACE PROCEDURE p_demo(p_id NUMBER) IS\n  v_id NUMBER;\n  v_estado cab.estado%TYPE;\nBEGIN\n  IF p_id IS NULL THEN\n    RAISE_APPLICATION_ERROR(-20999,'Debe declarar');\n  END IF;\n  BEGIN\n    SELECT estado\n    INTO v_estado\n    FROM cab\n    WHERE id_pedido = p_id\n      AND id_empresa = v_id;\n  EXCEPTION\n    WHEN NO_DATA_FOUND THEN\n      RAISE_APPLICATION_ERROR(-20999,'No existe');\n  END;\n  FOR r IN (SELECT 1 FROM dual) LOOP\n    NULL;\n  END LOOP;\nEND;\n/\n";
        assert_eq!(format_sql(sql, &standard()), expected);
    }

    #[test]
    fn plsql_alignment_matches_cpp() {
        let sql = "declare\nv_id number;\nv_estado_largo varchar2(10);\nbegin\nv_id := 1;\nv_estado_largo := 'x';\nexception\nwhen others then\nnull;\nend;";
        let options = FormatOptions {
            align_declarations: true,
            align_assignments: true,
            blank_after_declare: true,
            blank_before_exception: true,
            ..standard()
        };
        let expected = "DECLARE\n  v_id           NUMBER;\n  v_estado_largo VARCHAR2(10);\n\nBEGIN\n  v_id           := 1;\n  v_estado_largo := 'x';\n\nEXCEPTION\n  WHEN OTHERS THEN\n    NULL;\nEND;\n";
        assert_eq!(format_sql(sql, &options), expected);
    }

    #[test]
    fn custom_structure_matches_cpp() {
        let options = FormatOptions {
            cont_indent: Some(7),
            and_or_indent: Some(0),
            kw_alone_when_many: false,
            ..standard()
        };
        assert_eq!(
            format_sql("select a,b from t where x=1 and y=2", &options),
            "SELECT a,\n       b\nFROM t\nWHERE x = 1\nAND y = 2"
        );
    }

    #[test]
    fn join_indent_matches_cpp() {
        let options = FormatOptions {
            join_indent: Some(4),
            on_new_line: true,
            ..standard()
        };
        assert_eq!(
            format_sql("select a from t join u on t.id=u.id and t.x=u.x", &options),
            "SELECT a\nFROM t\n    JOIN u\n      ON t.id = u.id\n        AND t.x = u.x"
        );
    }

    #[test]
    fn comma_and_paren_spacing_matches_cpp() {
        let options = FormatOptions {
            space_after_comma: false,
            space_in_parens: true,
            ..standard()
        };
        assert_eq!(
            format_sql("select 1 from t where a in (1,2)", &options),
            "SELECT 1\nFROM t\nWHERE a IN ( 1,2 )"
        );
    }

    #[test]
    fn lowercase_keywords_matches_cpp() {
        let options = FormatOptions {
            keyword_case: KeywordCase::Lower,
            ..standard()
        };
        assert_eq!(
            format_sql("SELECT A FROM T WHERE X=1", &options),
            "select A\nfrom T\nwhere X = 1"
        );
    }

    #[test]
    fn profile_json_roundtrip() {
        let options = FormatOptions {
            indent: 3,
            style: FormatStyle::River,
            comma_leading: true,
            and_or_indent: Some(2),
            align_declarations: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&options).expect("serialize");
        assert!(json.contains("\"style\":\"River\""), "{json}");
        assert!(json.contains("\"commaLeading\":true"), "{json}");
        assert!(json.contains("\"andOrIndent\":2"), "{json}");
        let back: FormatOptions = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, options);
        let partial: FormatOptions =
            serde_json::from_str("{\"indent\":4}").expect("partial profile");
        assert_eq!(partial.indent, 4);
        assert_eq!(partial.style, FormatStyle::Standard);
        assert_eq!(partial.keyword_case, KeywordCase::Upper);
    }

    #[test]
    fn infers_river_profile() {
        let options = infer_from_example(
            "select a,\n    b\n  from t\n where x = 1\n   and y = 2\n   and z = 3;",
        );
        assert_eq!(options.style, FormatStyle::River);
        assert_eq!(options.keyword_case, KeywordCase::Lower);
    }

    #[test]
    fn infers_standard_profile() {
        let options = infer_from_example(
            "SELECT estado\nINTO v_estado\nFROM cab_pedido_venta\nWHERE id_pedido = p_id\nAND id_empresa = v_id\nAND x = 1;\nBEGIN\n    IF a THEN\n        b := 1;\n    END IF;\nEND;",
        );
        assert_eq!(options.style, FormatStyle::Standard);
        assert_eq!(options.and_or_indent, Some(0));
        assert_eq!(options.indent, 4);
        assert_eq!(options.keyword_case, KeywordCase::Upper);
        assert!(!options.kw_alone_when_many);
    }

    #[test]
    fn reserved_words_cover_the_original_set() {
        let output = format_sql("select level, prior from t", &standard());
        assert!(output.contains("LEVEL"), "{output}");
        assert!(output.contains("PRIOR"), "{output}");
        let output = format_sql("select * from t for update nowait", &standard());
        assert!(output.contains("NOWAIT"), "{output}");
    }

    #[test]
    fn infers_standard_for_indented_plsql() {
        // SELECT and FROM sit on the same column: Standard even though both are
        // indented (the old rule treated any indented clause as River).
        let options = infer_from_example("BEGIN\n    SELECT a\n    FROM t\n    WHERE x = 1;\nEND;");
        assert_eq!(options.style, FormatStyle::Standard);
    }

    #[test]
    fn infers_indent_with_tabs_as_four_columns() {
        let options = infer_from_example("BEGIN\n\tIF a THEN\n\t\tb := 1;\n\tEND IF;\nEND;");
        assert_eq!(options.indent, 4);
        assert!(options.use_tabs);
    }

    #[test]
    fn infers_join_on_and_spacing() {
        let sample = "SELECT a,\n       b\n  FROM t\n    JOIN u\n      ON t.id=u.id\n WHERE x IN ( 1,2 )\n   AND y = 1;";
        let options = infer_from_example(sample);
        assert_eq!(options.join_indent, Some(2));
        assert!(options.on_new_line);
        assert_eq!(options.and_or_indent, Some(2));
        assert!(!options.space_after_comma);
        assert!(options.space_in_parens);
        assert_eq!(options.style, FormatStyle::River);
    }

    #[test]
    fn infers_leading_comma_and_aligned_declarations() {
        let sample = "SELECT a\n       , b\n  FROM t;\nDECLARE\n  v_id     NUMBER;\n  v_total  NUMBER;\nBEGIN\n  NULL;\nEND;";
        let options = infer_from_example(sample);
        assert!(options.comma_leading);
        assert!(options.align_declarations);
        assert!(options.space_after_comma);
        assert!(!options.space_in_parens);
        assert!(!options.on_new_line);
    }

    #[test]
    fn infers_case_by_counting_reserved_words() {
        // The first keyword is lowercase, but most reserved words are uppercase.
        let options = infer_from_example("select a\nFROM t\nWHERE x = 1\nAND y = 2");
        assert_eq!(options.keyword_case, KeywordCase::Upper);
        let options = infer_from_example("SELECT a\nfrom t\nwhere x = 1\nand y = 2");
        assert_eq!(options.keyword_case, KeywordCase::Lower);
    }

    #[test]
    fn infers_indent_by_frequent_delta() {
        // The 1-space comma line must not win over the two 4-space steps.
        let options =
            infer_from_example("SELECT a\n , b\nFROM t\n    WHERE x = 1\n        AND y = 2");
        assert_eq!(options.indent, 4);
        assert!(options.comma_leading);
    }

    #[test]
    fn river_max_line_wrap_uses_clause_column() {
        let options = FormatOptions {
            style: FormatStyle::River,
            max_line: 39,
            ..standard()
        };
        assert_eq!(
            format_sql(
                "select 1 from t where a in (1111111111,2222222222,3333333333)",
                &options
            ),
            "SELECT 1\n  FROM t\n WHERE a IN (\n   1111111111,\n   2222222222,\n   3333333333\n )"
        );
        let once = format_sql(
            "select 1 from t where a in (1111111111,2222222222,3333333333)",
            &options,
        );
        assert_eq!(format_sql(&once, &options), once);
    }

    #[test]
    fn subquery_in_expression_is_indented() {
        assert_eq!(
            format_sql(
                "select x from t where a in (select id from u where b = 1)",
                &standard()
            ),
            "SELECT x\nFROM t\nWHERE a IN (\n  SELECT id\n  FROM u\n  WHERE b = 1\n)"
        );
    }

    #[test]
    fn line_comment_forces_a_new_line() {
        let output = format_sql("select f(a -- note\n, b) from t", &standard());
        assert!(output.contains("-- note\n"), "{output}");
        assert!(!output.contains("-- note,"), "{output}");
    }

    #[test]
    fn max_line_splits_long_paren_groups() {
        // "WHERE a IN (…)" is 40 columns wide; the rule breaks when it exceeds,
        // so 39 forces the split and 40 keeps it inline (`> o.maxLine`).
        let options = FormatOptions {
            max_line: 39,
            ..standard()
        };
        assert_eq!(
            format_sql(
                "select 1 from t where a in (1111111111,2222222222,3333333333)",
                &options
            ),
            "SELECT 1\nFROM t\nWHERE a IN (\n  1111111111,\n  2222222222,\n  3333333333\n)"
        );
        // Short groups stay on one line.
        assert_eq!(
            format_sql("select 1 from t where a in (1,2)", &options),
            "SELECT 1\nFROM t\nWHERE a IN (1, 2)"
        );
        // Re-formatting the wrapped output is stable.
        let once = format_sql(
            "select 1 from t where a in (1111111111,2222222222,3333333333)",
            &options,
        );
        assert_eq!(format_sql(&once, &options), once);
    }

    #[test]
    fn and_or_new_line_false_keeps_conditions_inline() {
        let options = FormatOptions {
            and_or_new_line: false,
            ..standard()
        };
        assert_eq!(
            format_sql("select a from t where x=1 and y=2", &options),
            "SELECT a\nFROM t\nWHERE x = 1 AND y = 2"
        );
    }

    #[test]
    fn one_item_per_line_false_starts_on_the_keyword_line() {
        let options = FormatOptions {
            one_item_per_line: false,
            ..standard()
        };
        assert_eq!(
            format_sql("select a,b from t", &options),
            "SELECT a,\n  b\nFROM t"
        );
    }

    #[test]
    fn river_style_matches_cpp() {
        let sql = "select a.id,b.nombre,sum(c.total) from cab_pedido a join cliente b on a.id_cliente=b.id left join det_pedido c on c.id_pedido=a.id where a.estado='A' and b.activo=1 group by a.id,b.nombre order by 3 desc;";
        let options = FormatOptions {
            style: FormatStyle::River,
            ..standard()
        };
        let expected = "SELECT a.id,\n       b.nombre,\n       SUM(c.total)\n  FROM cab_pedido a\n  JOIN cliente b ON a.id_cliente = b.id\n  LEFT JOIN det_pedido c ON c.id_pedido = a.id\n WHERE a.estado = 'A'\n   AND b.activo = 1\n GROUP BY a.id,\n          b.nombre\n ORDER BY 3 DESC;";
        assert_eq!(format_sql(sql, &options), expected);
    }

    #[test]
    fn formats_from_subquery() {
        assert_eq!(
            format_sql(
                "select x from (select id x from t where a in (1,2)) where x>0",
                &standard()
            ),
            "SELECT x\nFROM (\n  SELECT id x\n  FROM t\n  WHERE a IN (1, 2)\n)\nWHERE x > 0"
        );
    }

    #[test]
    fn insert_update_delete() {
        let sql = "insert into t(a,b) values(1,'x');update t set a=1,b=2 where id=3;delete from t where id=3";
        assert_eq!(
            format_sql(sql, &standard()),
            "INSERT INTO t (a, b)\nVALUES (1, 'x');\nUPDATE t\nSET\n  a = 1,\n  b = 2\nWHERE id = 3;\nDELETE FROM t\nWHERE id = 3"
        );
    }

    #[test]
    fn custom_indent_and_comma_leading() {
        let mut options = FormatOptions {
            indent: 4,
            ..standard()
        };
        assert_eq!(
            format_sql("select a from t where x=1 and y=2", &options),
            "SELECT a\nFROM t\nWHERE x = 1\n    AND y = 2"
        );
        options.indent = 2;
        options.comma_leading = true;
        assert_eq!(
            format_sql("select a,b,c from t", &options),
            "SELECT\n  a\n  , b\n  , c\nFROM t"
        );
    }

    #[test]
    fn between_does_not_split_the_and() {
        assert_eq!(
            format_sql(
                "select 1 from t where a between 1 and 5 and b=2",
                &standard()
            ),
            "SELECT 1\nFROM t\nWHERE a BETWEEN 1 AND 5\n  AND b = 2"
        );
    }

    #[test]
    fn operators_and_negatives() {
        assert_eq!(
            format_sql("select -1+2*3,a||b,count(*),t.* from t", &standard()),
            "SELECT\n  -1 + 2 * 3,\n  a || b,\n  COUNT(*),\n  t.*\nFROM t"
        );
    }

    #[test]
    fn preserves_strings_and_comments() {
        let sql = "select 'a,b  from' x, \"Mi Col\" -- nota, from\nfrom t";
        assert_eq!(
            format_sql(sql, &standard()),
            "SELECT\n  'a,b  from' x,\n  \"Mi Col\" -- nota, from\nFROM t"
        );
    }

    #[test]
    fn function_and_identifier_case_matches_cpp() {
        let options = FormatOptions {
            function_case: KeywordCase::Lower,
            identifier_case: KeywordCase::Lower,
            ..standard()
        };
        assert_eq!(
            format_sql("select NVL(A,B) from MI_TABLA", &options),
            "SELECT nvl(a, b)\nFROM mi_tabla"
        );
    }

    #[test]
    fn is_idempotent() {
        let first = format_sql("select a,b from t where x=1 and y=2", &standard());
        assert_eq!(format_sql(&first, &standard()), first);
    }
}
