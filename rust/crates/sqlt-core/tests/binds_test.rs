// SPDX-License-Identifier: GPL-3.0-only
// Paridad con tests/bind_test.cpp del port C++/Qt.

use std::collections::HashMap;

use sqlt_core::binds::{names, substitute, BindKind, BindValue};

fn values() -> HashMap<String, BindValue> {
    let mut v = HashMap::new();
    let mut put = |k: &str, kind: BindKind, value: &str| {
        v.insert(
            k.to_string(),
            BindValue {
                kind,
                value: value.to_string(),
            },
        );
    };
    put("N", BindKind::Auto, "42");
    put("T", BindKind::Auto, "O'Brien");
    put("D", BindKind::Date, "2026-03-15 13:45:09");
    put("D2", BindKind::Date, "2026-03-15");
    put("Z", BindKind::Null, "x");
    put("S", BindKind::String, "123");
    put("E", BindKind::Auto, "");
    put("BAD", BindKind::Number, "abc");
    put("BD", BindKind::Date, "15/03/2026");
    put("F", BindKind::Auto, "-1.5e3");
    v
}

#[test]
fn unique_names_in_order() {
    assert_eq!(
        names("select * from t where a = :a and b = :b or c = :a"),
        vec!["a", "b"]
    );
    assert_eq!(
        names("select ':x' , q'[:y]' from t -- :z\n where /* :w */ a = :ok"),
        vec!["ok"]
    );
    assert!(names("begin v := 1; x := y::z; end;").is_empty());
    assert_eq!(names("insert into t values (:1, :2)"), vec!["1", "2"]);
    assert!(names("select \":x\" from t").is_empty());
}

#[test]
fn auto_and_text_substitution() {
    let (sql, warnings) = substitute("a = :n and b = :t", &values());
    assert_eq!(sql, "a = 42 and b = 'O''Brien'");
    assert!(warnings.is_empty());
}

#[test]
fn dates_with_and_without_time() {
    let (sql, _) = substitute(":d / :d2", &values());
    assert_eq!(
        sql,
        "to_date('2026-03-15 13:45:09', 'YYYY-MM-DD HH24:MI:SS') / to_date('2026-03-15', 'YYYY-MM-DD')"
    );
}

#[test]
fn null_empty_and_numeric_string() {
    let (sql, _) = substitute(":z|:e|:s", &values());
    assert_eq!(sql, "NULL|NULL|'123'");
}

#[test]
fn scientific_notation_is_number() {
    let (sql, _) = substitute(":f", &values());
    assert_eq!(sql, "-1.5e3");
}

#[test]
fn invalid_values_become_null_with_warning() {
    let (sql, warnings) = substitute(":bad :bd", &values());
    assert_eq!(sql, "NULL NULL");
    assert_eq!(warnings.len(), 2);
}

#[test]
fn does_not_touch_text_or_comments() {
    let (sql, _) = substitute("select ':n' x, :n y -- :n\n from t", &values());
    assert_eq!(sql, "select ':n' x, 42 y -- :n\n from t");
}

#[test]
fn missing_variable_is_null() {
    let (sql, _) = substitute("a = :nueva", &values());
    assert_eq!(sql, "a = NULL");
}

#[test]
fn names_are_case_insensitive() {
    let (sql, _) = substitute("a = :N and b = :n", &values());
    assert_eq!(sql, "a = 42 and b = 42");
}

#[test]
fn positional_binds_are_substituted() {
    let mut v = HashMap::new();
    v.insert(
        "1".to_string(),
        BindValue {
            kind: BindKind::Auto,
            value: "10".to_string(),
        },
    );
    let (sql, _) = substitute("insert into t values (:1, :2)", &v);
    assert_eq!(sql, "insert into t values (10, NULL)");
}
