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
    String::from_utf16(&units[start..end]).expect("rango UTF-16 válido")
}

#[test]
fn dos_selects() {
    expect(
        "dos selects",
        "select 1 from dual;\nselect 2 from dual;",
        &["select 1 from dual", "select 2 from dual"],
        &[],
    );
}

#[test]
fn punto_y_coma_en_texto() {
    expect(
        "punto y coma en texto",
        "select 'a;b' x from dual; select 2 from dual",
        &["select 'a;b' x from dual", "select 2 from dual"],
        &[],
    );
}

#[test]
fn comilla_escapada() {
    expect(
        "comilla escapada",
        "select 'it''s; ok' from dual;",
        &["select 'it''s; ok' from dual"],
        &[],
    );
}

#[test]
fn punto_y_coma_en_comentario_de_linea() {
    expect(
        "punto y coma en comentario de línea",
        "select 1 -- uno; dos\nfrom dual;",
        &["select 1 -- uno; dos\nfrom dual"],
        &[],
    );
}

#[test]
fn punto_y_coma_en_comentario_de_bloque() {
    expect(
        "punto y coma en comentario de bloque",
        "select /* a; b */ 1 from dual;",
        &["select /* a; b */ 1 from dual"],
        &[],
    );
}

#[test]
fn literal_q_con_corchetes() {
    expect(
        "literal q'[ ]'",
        "select q'[a;b]' from dual; select 2 from dual",
        &["select q'[a;b]' from dual", "select 2 from dual"],
        &[],
    );
}

#[test]
fn literal_q_con_llaves() {
    expect(
        "literal q'{ }'",
        "select q'{a;b}' from dual; select 2 from dual",
        &["select q'{a;b}' from dual", "select 2 from dual"],
        &[],
    );
}

#[test]
fn identificador_entrecomillado() {
    expect(
        "identificador entrecomillado",
        "select \"a;b\" from dual; select 2 from dual",
        &["select \"a;b\" from dual", "select 2 from dual"],
        &[],
    );
}

#[test]
fn bloque_anonimo_simple() {
    expect(
        "bloque anónimo simple",
        "begin null; end;\nselect 1 from dual;",
        &["begin null; end;", "select 1 from dual"],
        &[true, false],
    );
}

#[test]
fn bloque_con_if_loop_case() {
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
        "bloque con if/loop/case",
        script,
        &[first, "select 1 from dual"],
        &[true, false],
    );
}

#[test]
fn declare_y_begin() {
    expect(
        "declare + begin",
        "declare v number; begin v := 1; end;\nselect 2 from dual;",
        &["declare v number; begin v := 1; end;", "select 2 from dual"],
        &[true, false],
    );
}

#[test]
fn bloques_anidados() {
    expect(
        "bloques anidados",
        "begin begin null; end; null; end;\nselect 1 from dual;",
        &["begin begin null; end; null; end;", "select 1 from dual"],
        &[],
    );
}

#[test]
fn create_procedure_con_barra() {
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
        "create procedure con /",
        script,
        &[first, "select 1 from dual"],
        &[true, false],
    );
}

#[test]
fn create_package_y_body() {
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
        "create package y body",
        script,
        &[spec, body],
        &[true, true],
    );
}

#[test]
fn create_table_es_sql_suelto() {
    expect(
        "create table es SQL suelto",
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
fn sin_punto_y_coma_final() {
    expect(
        "sin punto y coma final",
        "select 1 from dual",
        &["select 1 from dual"],
        &[],
    );
}

#[test]
fn comentarios_al_inicio() {
    expect(
        "comentarios al inicio",
        "-- hola\nselect 1 from dual;\n/* x */ select 2 from dual;",
        &["select 1 from dual", "select 2 from dual"],
        &[],
    );
}

#[test]
fn barra_sola_cierra_sql() {
    expect(
        "barra sola cierra sql",
        "select 1 from dual\n/\nselect 2 from dual\n/",
        &["select 1 from dual", "select 2 from dual"],
        &[],
    );
}

#[test]
fn script_vacio() {
    expect("vacío", "  \n ; \n", &[], &[]);
}

#[test]
fn solo_comentarios_no_produce_sentencias() {
    expect("solo comentarios", "-- hola\n/* x */\n", &[], &[]);
}

#[test]
fn statement_at_igual_al_original() {
    let v = split("select 1 from dual;\n\nselect 2 from dual;");
    assert_eq!(statement_at(&v, 5), Some(0));
    assert_eq!(statement_at(&v, 20), Some(0));
    assert_eq!(statement_at(&v, 21), Some(1));
    assert_eq!(statement_at(&v, 19), Some(0));
    assert_eq!(statement_at(&[], 0), None);
}

#[test]
fn offsets_son_utf16_con_acentos_y_emoji() {
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
fn end_coincide_con_el_texto_en_sql_y_plsql() {
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
fn crlf_conserva_texto_y_lineas() {
    let got = split("select 1 from dual;\r\nselect 2 from dual;\r\n");
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].text, "select 1 from dual");
    assert_eq!(got[1].text, "select 2 from dual");
    assert_eq!(got[1].start_line, 2);
}
