// SPDX-License-Identifier: GPL-3.0-only
//! SQL formatter: clause layout, one item per line, JOIN/ON, AND/OR, operator
//! spacing and comma position. Golden cases come from `tests/format_test.cpp`.
//!
//! PL/SQL blocks use a conservative keyword/indentation pass; full PL/SQL
//! re-indentation parity is still incremental.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatStyle {
    Standard,
    River,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatOptions {
    pub uppercase_keywords: bool,
    pub indent: usize,
    pub comma_leading: bool,
    pub style: FormatStyle,
}

impl Default for FormatOptions {
    fn default() -> Self {
        FormatOptions {
            uppercase_keywords: true,
            indent: 2,
            comma_leading: false,
            style: FormatStyle::Standard,
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
    "CROSS",
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
    "INTO",
    "ASC",
    "DESC",
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
        "SELECT" | "FROM" | "WHERE" | "HAVING" | "SET" | "VALUES" | "UPDATE" | "MERGE" => {
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
    let mut out = String::new();
    let mut tight = false;
    for (index, token) in tokens.iter().enumerate() {
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
                let is_function = matches!(tokens.get(index + 1), Some(Token::Symbol('(')));
                if options.uppercase_keywords && (KEYWORDS.contains(&upper.as_str()) || is_function)
                {
                    out.push_str(&upper);
                } else {
                    out.push_str(word);
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
            }
            Token::Symbol('(') => {
                if should_space_before_paren(tokens.get(index.wrapping_sub(1))) {
                    out.push(' ');
                }
                out.push('(');
            }
            Token::Symbol(')') => out.push(')'),
            Token::Symbol(',') => out.push_str(", "),
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
    }
    out.trim().to_string()
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

fn river_prefix(keyword: &str) -> String {
    match keyword {
        "SELECT" => "SELECT ".to_string(),
        "GROUP BY" => " GROUP BY ".to_string(),
        "ORDER BY" => " ORDER BY ".to_string(),
        "SET" => "  SET ".to_string(),
        _ => format!("{keyword} "),
    }
}

fn format_list(keyword: &str, body: &[Token], options: &FormatOptions) -> String {
    let items = split_commas(body);
    let river = options.style == FormatStyle::River;
    let prefix = if river {
        river_prefix(keyword)
    } else {
        format!("{keyword} ")
    };
    if items.len() <= 1 {
        return format!("{prefix}{}", render(body, options));
    }
    if river {
        let pad = " ".repeat(prefix.len());
        let mut out = prefix.clone();
        for (index, item) in items.iter().enumerate() {
            if index > 0 {
                out.push('\n');
                out.push_str(&pad);
            }
            out.push_str(&render(item, options));
            if index + 1 < items.len() {
                out.push(',');
            }
        }
        return out;
    }
    let indent = " ".repeat(options.indent);
    let mut lines = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let text = render(item, options);
        if options.comma_leading {
            if index == 0 {
                lines.push(format!("{indent}{text}"));
            } else {
                lines.push(format!("{indent}, {text}"));
            }
        } else {
            let comma = if index + 1 < items.len() { "," } else { "" };
            lines.push(format!("{indent}{text}{comma}"));
        }
    }
    format!("{keyword}\n{}", lines.join("\n"))
}

fn format_where(body: &[Token], options: &FormatOptions) -> String {
    let indent = " ".repeat(options.indent);
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
    let river = options.style == FormatStyle::River;
    let prefix = if river { " WHERE " } else { "WHERE " };
    let and_indent = if river { "   " } else { &indent };
    let mut out = format!("{prefix}{}", render(&parts[0], options));
    for part in parts.iter().skip(1) {
        out.push_str(&format!("\n{and_indent}{}", render(part, options)));
    }
    out
}

fn format_from(body: &[Token], options: &FormatOptions) -> String {
    let indent = " ".repeat(options.indent);
    let river = options.style == FormatStyle::River;
    let prefix = if river { "  FROM " } else { "FROM " };
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
    let join_indent = if river { "  ".to_string() } else { indent };
    let mut out = format!("{prefix}{}", render(&joins[0], options));
    for join in joins.iter().skip(1) {
        out.push_str(&format!("\n{join_indent}{}", render(join, options)));
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
            "VALUES" => format!("VALUES {}", render(&body, options)),
            "INSERT INTO" => {
                let mut body_options = *options;
                body_options.uppercase_keywords = false;
                let rendered = render(&body, &body_options);
                let with_space = match rendered.find('(') {
                    Some(position) => {
                        format!("{} {}", &rendered[..position], &rendered[position..])
                    }
                    None => rendered,
                };
                format!("INSERT INTO {with_space}")
            }
            _ => format!("{clause} {}", render(&body, options)),
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

fn format_plsql(tokens: &[Token], options: &FormatOptions) -> String {
    // Conservative pass: uppercase keywords, keep line structure and indent.
    let mut lines: Vec<String> = Vec::new();
    let mut current: Vec<Token> = Vec::new();
    for token in tokens {
        if *token == Token::Symbol(';') {
            current.push(token.clone());
            lines.push(render(&current, options));
            current.clear();
            continue;
        }
        if let Token::Word(word) = token {
            let upper = word.to_ascii_uppercase();
            if matches!(
                upper.as_str(),
                "BEGIN" | "DECLARE" | "END" | "IF" | "LOOP" | "EXCEPTION"
            ) && !current.is_empty()
            {
                lines.push(render(&current, options));
                current.clear();
            }
        }
        current.push(token.clone());
    }
    if !current.is_empty() {
        lines.push(render(&current, options));
    }
    lines.join("\n")
}

pub fn format_sql(input: &str, options: &FormatOptions) -> String {
    let tokens = tokenize(input);
    if tokens.is_empty() {
        return String::new();
    }
    if is_plsql(&tokens) {
        return format_plsql(&tokens, options);
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
        let formatted = format_sql(sql, &standard());
        assert!(formatted.contains("'a,b  from'"));
        assert!(formatted.contains("\"Mi Col\""));
        assert!(formatted.contains("-- nota, from"));
        assert!(formatted.contains("FROM t"));
    }

    #[test]
    fn is_idempotent() {
        let first = format_sql("select a,b from t where x=1 and y=2", &standard());
        assert_eq!(format_sql(&first, &standard()), first);
    }
}
