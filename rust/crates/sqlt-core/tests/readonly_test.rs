// SPDX-License-Identifier: GPL-3.0-only
use sqlt_core::readonly::is_read_only;

#[test]
fn solo_lectura_acepta_consultas() {
    assert!(is_read_only("select 1 from dual"));
    assert!(is_read_only("  SELECT * FROM t"));
    assert!(is_read_only(
        "-- comentario\nwith x as (select 1 from dual) select * from x"
    ));
    assert!(is_read_only("/* c */ explain plan for select 1 from dual"));
    assert!(is_read_only("describe t"));
    assert!(is_read_only("desc t"));
    assert!(is_read_only(""));
}

#[test]
fn solo_lectura_bloquea_escrituras_y_plsql() {
    assert!(!is_read_only("insert into t values (1)"));
    assert!(!is_read_only("update t set a = 1"));
    assert!(!is_read_only("delete from t"));
    assert!(!is_read_only(
        "merge into t using dual on (1=1) when matched then update set a=1"
    ));
    assert!(!is_read_only("create table t (a number)"));
    assert!(!is_read_only("drop table t"));
    assert!(!is_read_only("begin null; end;"));
    assert!(!is_read_only("declare v number; begin null; end;"));
    assert!(!is_read_only("truncate table t"));
    assert!(!is_read_only("grant select on t to u"));
}
