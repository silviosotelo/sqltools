// SPDX-License-Identifier: GPL-3.0-only
// Ported from tests/meta_test.cpp plus formatting rules from MetaObjects.cpp.

use sqlt_meta::{Constraint, Grant, Index, TabColumn, Table, Trigger, WriteSettings};

fn column(
    name: &str,
    data_type: &str,
    data_length: i64,
    precision: i64,
    scale: i64,
    nullable: bool,
    data_default: &str,
) -> TabColumn {
    TabColumn {
        name: name.to_string(),
        data_type: data_type.to_string(),
        data_length,
        precision,
        scale,
        nullable,
        data_default: data_default.to_string(),
        comments: None,
    }
}

fn table() -> Table {
    let mut table = Table::new("ANAMNESIS", "CLIENTE");
    table.columns = vec![
        column("ID_CLIENTE", "NUMBER", 22, 10, 0, false, ""),
        column("CI", "VARCHAR2", 50, 0, 0, false, ""),
        column("RUC", "VARCHAR2", 50, 0, 0, true, ""),
        column("SEXO", "CHAR", 10, 0, 0, false, ""),
        column("MINUSVALIDO", "CHAR", 2, 0, 0, true, "'NO'"),
        column("FECHA_ALTA", "DATE", 7, 0, 0, true, ""),
        column("\"COMMENT\"", "VARCHAR2", 100, 0, 0, true, ""),
    ];
    table
}

fn settings(storage: bool) -> WriteSettings {
    WriteSettings {
        generate_prompts: false,
        storage_clause: storage,
        lower_db_name: true,
        ..WriteSettings::default()
    }
}

#[test]
fn create_table_matches_windows() {
    let expected = "CREATE TABLE cliente (\n  id_cliente  NUMBER(10,0)  NOT NULL,\n  ci          VARCHAR2(50)  NOT NULL,\n  ruc         VARCHAR2(50)  NULL,\n  sexo        CHAR(10)      NOT NULL,\n  minusvalido CHAR(2)       DEFAULT 'NO' NULL,\n  fecha_alta  DATE          NULL,\n  \"COMMENT\"     VARCHAR2(100) NULL\n)\n/\n\n";
    assert_eq!(table().write_definition(&settings(false)), expected);
}

#[test]
fn storage_matches_windows() {
    let mut table = table();
    table.tablespace = Some("FACTURACION_DATA".to_string());
    table.next_extent = Some(1024 * 1024);
    let ddl = table.write_definition(&settings(true));
    let suffix = &ddl[ddl.find(")\n").expect("closing parenthesis")..];
    assert_eq!(
        suffix,
        ")\n  TABLESPACE facturacion_data\n  STORAGE (\n    NEXT       1024 K\n  )\n/\n\n"
    );
}

#[test]
fn trigger_header_matches_windows() {
    let trigger = Trigger {
        owner: "ANAMNESIS".to_string(),
        name: "CLIENTE_DESP_TRIG".to_string(),
        table_owner: "ANAMNESIS".to_string(),
        table_name: "CLIENTE".to_string(),
        description: "CLIENTE_DESP_TRIG \nAFTER INSERT OR DELETE OR UPDATE ON CLIENTE \nREFERENCING NEW AS NEW OLD AS OLD \nFOR EACH ROW\n".to_string(),
        trigger_body: "BEGIN\n  NULL;\nEND;".to_string(),
    };
    let output = trigger.write(&settings(false));
    let header = &output[..output.find("FOR EACH ROW").expect("header")];
    assert_eq!(
        header,
        "CREATE OR REPLACE TRIGGER cliente_desp_trig\nAFTER INSERT OR DELETE OR UPDATE ON cliente\nREFERENCING NEW AS NEW OLD AS OLD\n"
    );
}

#[test]
fn indexes_constraints_comments_grants() {
    let mut table = table();
    table.comments = Some("Tabla de clientes".to_string());
    table.columns[1].comments = Some("Cédula".to_string());
    table.indexes.push(Index {
        name: "IDX_CLIENTE_CI".to_string(),
        unique: true,
        bitmap: false,
        columns: vec!["CI".to_string()],
        tablespace: None,
    });
    table.constraints.push(Constraint {
        name: "PK_CLIENTE".to_string(),
        kind: 'P',
        columns: vec!["ID_CLIENTE".to_string()],
        condition: None,
        ref_table: None,
        ref_columns: Vec::new(),
        delete_rule: None,
    });
    table.grants.push(Grant {
        grantee: "REPORTE".to_string(),
        privileges: vec!["SELECT".to_string(), "UPDATE".to_string()],
        grantable: false,
    });

    let output = table.write_full(&settings(false));
    assert!(
        output.contains("CREATE UNIQUE INDEX idx_cliente_ci\n  ON cliente (\n    ci\n  )\n/\n\n")
    );
    assert!(output.contains(
        "ALTER TABLE cliente\n  ADD CONSTRAINT pk_cliente PRIMARY KEY (\n    id_cliente\n  )\n/\n\n"
    ));
    assert!(output.contains("COMMENT ON TABLE cliente IS 'Tabla de clientes';\n\n"));
    assert!(output.contains("COMMENT ON COLUMN cliente.ci IS 'Cédula';\n"));
    assert!(output.contains("GRANT SELECT, UPDATE ON cliente TO reporte;\n"));
}
