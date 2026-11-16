// SPDX-License-Identifier: GPL-3.0-only
// Paridad con tests/split_test.cpp del port C++/Qt.

use sqlt_core::{split, statement_at};

fn expect(name: &str, script: &str, want: &[&str], plsql: &[bool]) {
    let got = split(script);
    assert_eq!(got.len(), want.len(), "{name}: cantidad de sentencias");
    for (i, w) in want.iter().enumerate() {
        assert_eq!(got[i].text, *w, "{name}: texto de la sentencia {i}");
        if let Some(p) = plsql.get(i) {
            assert_eq!(got[i].plsql, *p, "{name}: plsql de la sentencia {i}");
        }
    }
}

fn utf16_slice(s: &str, start: usize, end: usize) -> String {
    let units: Vec<u16> = s.encode_utf16().collect();
    String::from_utf16(&units[start..end]).expect("valid UTF-16 range")
}

#[test]
fn two_selects() {
    expect(
        "two selects",
        "select 1 from dual;\nselect 2 from dual;",
        &["select 1 from dual", "select 2 from dual"],
        &[],
    );
}

#[test]
fn semicolon_in_text() {
    expect(
        "semicolon in text",
        "select 'a;b' x from dual; select 2 from dual",
        &["select 'a;b' x from dual", "select 2 from dual"],
        &[],
    );
}

#[test]
fn escaped_quote() {
    expect(
        "escaped quote",
        "select 'it''s; ok' from dual;",
        &["select 'it''s; ok' from dual"],
        &[],
    );
}

#[test]
fn semicolon_in_line_comment() {
    expect(
        "semicolon in line comment",
        "select 1 -- uno; dos\nfrom dual;",
        &["select 1 -- uno; dos\nfrom dual"],
        &[],
    );
}

#[test]
fn semicolon_in_block_comment() {
    expect(
        "semicolon in block comment",
        "select /* a; b */ 1 from dual;",
        &["select /* a; b */ 1 from dual"],
        &[],
    );
}

#[test]
fn q_literal_with_brackets() {
    expect(
        "q'[ ]' literal",
        "select q'[a;b]' from dual; select 2 from dual",
        &["select q'[a;b]' from dual", "select 2 from dual"],
        &[],
    );
}

#[test]
fn q_literal_with_braces() {
    expect(
        "q'{ }' literal",
        "select q'{a;b}' from dual; select 2 from dual",
        &["select q'{a;b}' from dual", "select 2 from dual"],
        &[],
    );
}

#[test]
fn quoted_identifier() {
    expect(
        "quoted identifier",
        "select \"a;b\" from dual; select 2 from dual",
        &["select \"a;b\" from dual", "select 2 from dual"],
        &[],
    );
}

#[test]
fn simple_anonymous_block() {
    expect(
        "simple anonymous block",
        "begin null; end;\nselect 1 from dual;",
        &["begin null; end;", "select 1 from dual"],
        &[true, false],
    );
}

#[test]
fn block_with_if_loop_case() {
    let script = r#"begin
  if x then
    for i in 1..3 loop
      null;
    end loop;
  elsif y then
    case when a then null; end case;
  end if;
  v := case when 1=1 then 1 else 2 end;
end;
select 1 from dual;"#;
    let first = r#"begin
  if x then
    for i in 1..3 loop
      null;
    end loop;
  elsif y then
    case when a then null; end case;
  end if;
  v := case when 1=1 then 1 else 2 end;
end;"#;
    expect(
        "block with if/loop/case",
        script,
        &[first, "select 1 from dual"],
        &[true, false],
    );
}

#[test]
fn declare_and_begin() {
    expect(
        "declare + begin",
        "declare v number; begin v := 1; end;\nselect 2 from dual;",
        &["declare v number; begin v := 1; end;", "select 2 from dual"],
        &[true, false],
    );
}

#[test]
fn nested_blocks() {
    expect(
        "nested blocks",
        "begin begin null; end; null; end;\nselect 1 from dual;",
        &["begin begin null; end; null; end;", "select 1 from dual"],
        &[],
    );
}

#[test]
fn create_procedure_with_slash() {
    let script = r#"create or replace procedure p is
begin
  null;
end;
/
select 1 from dual;"#;
    let first = r#"create or replace procedure p is
begin
  null;
end;"#;
    expect(
        "create procedure with /",
        script,
        &[first, "select 1 from dual"],
        &[true, false],
    );
}

#[test]
fn create_package_and_body() {
    let script = r#"create or replace package pk as
  procedure a;
end pk;
/
create or replace package body pk as
  procedure a is begin null; end;
end pk;
/"#;
    let spec = r#"create or replace package pk as
  procedure a;
end pk;"#;
    let body = r#"create or replace package body pk as
  procedure a is begin null; end;
end pk;"#;
    expect(
        "create package and body",
        script,
        &[spec, body],
        &[true, true],
    );
}

#[test]
fn create_table_is_plain_sql() {
    expect(
        "create table is plain SQL",
        "create table t (a number);\nselect 1 from dual;",
        &["create table t (a number)", "select 1 from dual"],
        &[false, false],
    );
}

#[test]
fn create_trigger() {
    let script = r#"create or replace trigger tg before insert on t for each row
begin
 :new.a := 1;
end;
/"#;
    let first = r#"create or replace trigger tg before insert on t for each row
begin
 :new.a := 1;
end;"#;
    expect("create trigger", script, &[first], &[true]);
}

#[test]
fn no_trailing_semicolon() {
    expect(
        "no trailing semicolon",
        "select 1 from dual",
        &["select 1 from dual"],
        &[],
    );
}

#[test]
fn leading_comments() {
    expect(
        "leading comments",
        "-- hola\nselect 1 from dual;\n/* x */ select 2 from dual;",
        &["select 1 from dual", "select 2 from dual"],
        &[],
    );
}

#[test]
fn standalone_slash_closes_sql() {
    expect(
        "standalone slash closes sql",
        "select 1 from dual\n/\nselect 2 from dual\n/",
        &["select 1 from dual", "select 2 from dual"],
        &[],
    );
}

#[test]
fn empty_script() {
    expect("empty", "  \n ; \n", &[], &[]);
}

#[test]
fn comments_only_produce_no_statements() {
    expect("comments only", "-- hola\n/* x */\n", &[], &[]);
}

#[test]
fn statement_at_matches_original() {
    let v = split("select 1 from dual;\n\nselect 2 from dual;");
    assert_eq!(statement_at(&v, 5), Some(0));
    assert_eq!(statement_at(&v, 20), Some(0));
    assert_eq!(statement_at(&v, 21), Some(1));
    assert_eq!(statement_at(&v, 19), Some(0));
    assert_eq!(statement_at(&[], 0), None);
}

#[test]
fn offsets_are_utf16_with_accents_and_emoji() {
    let sql = "select 'ñ😀' from dual;\nselect 2 from dual;";
    let got = split(sql);
    assert_eq!(got.len(), 2);
    assert_eq!(
        utf16_slice(sql, got[0].start, got[0].end),
        "select 'ñ😀' from dual"
    );
    assert_eq!(got[0].start_line, 1);
    assert_eq!(got[1].start_line, 2);
    assert_eq!(
        utf16_slice(sql, got[1].start, got[1].end),
        "select 2 from dual"
    );
}

#[test]
fn end_matches_text_in_sql_and_plsql() {
    for script in [
        "select 1 from dual;",
        "select 1 from dual",
        "begin null; end;",
        "declare v number; begin v := 1; end;",
        "begin\n  null;\nend;",
    ] {
        for st in split(script) {
            let units = st.text.encode_utf16().count();
            assert_eq!(st.end - st.start, units, "script {script:?}");
        }
    }
}

#[test]
fn crlf_preserves_text_and_lines() {
    let got = split("select 1 from dual;\r\nselect 2 from dual;\r\n");
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].text, "select 1 from dual");
    assert_eq!(got[1].text, "select 2 from dual");
    assert_eq!(got[1].start_line, 2);
}
