// SPDX-License-Identifier: GPL-3.0-only
use std::collections::HashMap;

use sqlt_oracle::{ConnectMode, ConnectSpec, ConnectTarget, SpecError};

fn spec(vars: &[(&str, &str)]) -> Result<ConnectSpec, SpecError> {
    let map: HashMap<String, String> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    ConnectSpec::from_lookup(move |k| map.get(k).cloned())
}

#[test]
fn alias_con_defaults() {
    let s = spec(&[
        ("SQLT_USER", "kmelot"),
        ("SQLT_PASS", "secreto"),
        ("SQLT_ALIAS", "santa"),
    ])
    .unwrap();
    assert_eq!(s.target, ConnectTarget::Alias("santa".to_string()));
    assert_eq!(s.mode, ConnectMode::Normal);
    assert!(!s.read_only);
    assert_eq!(s.prefetch, 1000);
    assert_eq!(s.connect_string(), "santa");
}

#[test]
fn conexion_directa_y_modo() {
    let s = spec(&[
        ("SQLT_USER", "sys"),
        ("SQLT_PASS", "x"),
        ("SQLT_CONNECT", "//10.0.0.5:1521/anam"),
        ("SQLT_MODE", "sysdba"),
        ("SQLT_READ_ONLY", "true"),
        ("SQLT_PREFETCH", "20000"),
    ])
    .unwrap();
    assert_eq!(
        s.target,
        ConnectTarget::Direct("//10.0.0.5:1521/anam".to_string())
    );
    assert_eq!(s.mode, ConnectMode::SysDba);
    assert!(s.read_only);
    assert_eq!(s.prefetch, 20000);
    assert_eq!(s.connect_string(), "//10.0.0.5:1521/anam");
}

#[test]
fn faltan_variables_obligatorias() {
    assert_eq!(spec(&[]).unwrap_err(), SpecError::Missing("SQLT_USER"));
    assert_eq!(
        spec(&[("SQLT_USER", "u")]).unwrap_err(),
        SpecError::Missing("SQLT_PASS")
    );
    assert_eq!(
        spec(&[("SQLT_USER", "u"), ("SQLT_PASS", "p")]).unwrap_err(),
        SpecError::Missing("SQLT_ALIAS o SQLT_CONNECT")
    );
}

#[test]
fn modo_invalido() {
    let err = spec(&[
        ("SQLT_USER", "u"),
        ("SQLT_PASS", "p"),
        ("SQLT_ALIAS", "a"),
        ("SQLT_MODE", "root"),
    ])
    .unwrap_err();
    assert_eq!(err, SpecError::InvalidMode("root".to_string()));
}

#[test]
fn parseo_de_modos() {
    assert_eq!(ConnectMode::parse(""), Some(ConnectMode::Normal));
    assert_eq!(ConnectMode::parse("normal"), Some(ConnectMode::Normal));
    assert_eq!(ConnectMode::parse("SYSDBA"), Some(ConnectMode::SysDba));
    assert_eq!(ConnectMode::parse(" SysOper "), Some(ConnectMode::SysOper));
    assert_eq!(ConnectMode::parse("otro"), None);
}

#[test]
fn direct_arma_la_cadena() {
    let s = ConnectSpec::direct("u", "p", "10.0.0.5", 1521, "anam");
    assert_eq!(s.connect_string(), "//10.0.0.5:1521/anam");
}
