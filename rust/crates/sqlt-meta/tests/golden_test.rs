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
        virtual_column: false,
    }
}

fn constraint(name: &str, kind: char, columns: &[&str]) -> Constraint {
    Constraint {
        name: name.to_string(),
        kind,
        generated: false,
        columns: columns.iter().map(|value| value.to_string()).collect(),
        condition: None,
        ref_table: None,
        ref_owner: None,
        ref_columns: Vec::new(),
        delete_rule: None,
        no_validate: false,
        deferrable: false,
        deferred: false,
        rely: false,
        disabled: false,
        using_index: None,
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
        when_clause: None,
        action_type: "PL/SQL".to_string(),
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
        next_extent: None,
        compress: None,
    });
    table
        .constraints
        .push(constraint("PK_CLIENTE", 'P', &["ID_CLIENTE"]));
    table.grants.push(Grant {
        grantee: "REPORTE".to_string(),
        privileges: vec!["SELECT".to_string(), "UPDATE".to_string()],
        grantable: false,
        column_privileges: Vec::new(),
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
    assert!(output.contains("GRANT SELECT,UPDATE ON cliente TO reporte;\n"));
}

#[test]
fn not_validated_constraint_prints_novalidate() {
    let mut check = constraint("CLI_COMISALBE", 'C', &[]);
    check.condition = Some("comision > 0".to_string());
    check.no_validate = true;
    assert_eq!(
        check.write(&table(), &settings(false)),
        "ALTER TABLE cliente\n  ADD CONSTRAINT cli_comisalbe CHECK (\n    comision > 0\n  )\n  NOVALIDATE\n/\n\n"
    );

    let validated = constraint("PK_CLIENTE", 'P', &["ID_CLIENTE"]);
    assert!(!validated
        .write(&table(), &settings(false))
        .contains("NOVALIDATE"));
}

#[test]
fn constraint_state_clauses_match_metobjects_order() {
    let mut key = constraint("PK_CLIENTE", 'P', &["ID_CLIENTE"]);
    key.deferrable = true;
    key.deferred = true;
    key.rely = true;
    key.disabled = true;
    key.no_validate = true;
    assert!(key
        .write(&table(), &settings(false))
        .ends_with("  )\n  INITIALLY DEFERRED DEFERRABLE\n  RELY\n  DISABLE\n  NOVALIDATE\n/\n\n"));

    // DEFERRABLE without DEFERRED prints no INITIALLY clause.
    let mut plain = constraint("PK_CLIENTE", 'P', &["ID_CLIENTE"]);
    plain.deferrable = true;
    let output = plain.write(&table(), &settings(false));
    assert!(output.ends_with("  )\n  DEFERRABLE\n/\n\n"));
    assert!(!output.contains("INITIALLY"));
}

#[test]
fn generated_constraint_name_is_not_printed() {
    let mut other = constraint("SYS_C009890", 'C', &["SEXO"]);
    other.generated = true;
    other.condition = Some("SEXO IN ('M', 'F')".to_string());
    assert_eq!(
        other.write(&table(), &settings(false)),
        "ALTER TABLE cliente\n  ADD CHECK (\n    SEXO IN ('M', 'F')\n  )\n/\n\n"
    );
}

#[test]
fn not_null_check_constraint_is_skipped() {
    let mut table = table();
    let mut not_null = constraint("SYS_C009889", 'C', &["CI"]);
    not_null.generated = true;
    not_null.condition = Some("CI IS NOT NULL".to_string());
    table.constraints.push(not_null);
    let mut real = constraint("CLI_SEXO_CK", 'C', &["SEXO"]);
    real.condition = Some("SEXO IN ('M', 'F')".to_string());
    table.constraints.push(real);
    let output = table.write_full(&settings(false));
    assert!(!output.contains("CI IS NOT NULL"), "{output}");
    assert!(
        output.contains("CONSTRAINT cli_sexo_ck CHECK ("),
        "{output}"
    );
}

#[test]
fn trigger_when_and_call_match_the_original() {
    let trigger = Trigger {
        owner: "ANAMNESIS".to_string(),
        name: "CLIENTE_BT".to_string(),
        table_owner: "ANAMNESIS".to_string(),
        table_name: "CLIENTE".to_string(),
        description: "CLIENTE_BT \nBEFORE INSERT ON CLIENTE \nFOR EACH ROW\n".to_string(),
        trigger_body: "pkg.audit(:NEW.id)".to_string(),
        when_clause: Some("NEW.id IS NOT NULL".to_string()),
        action_type: "CALL".to_string(),
    };
    assert_eq!(
        trigger.write(&settings(false)),
        "CREATE OR REPLACE TRIGGER cliente_bt\nBEFORE INSERT ON cliente\nFOR EACH ROW\nWHEN (NEW.id IS NOT NULL)\nCALL pkg.audit(:NEW.id)\n/\n\n"
    );
}

#[test]
fn cross_schema_foreign_key_qualifies_the_target() {
    let mut table = Table::new("ANAMNESIS", "CLIENTE");
    table.columns = vec![column("ID_PAIS", "NUMBER", 22, 4, 0, true, "")];

    let mut fk = constraint("CLIENTE_PAIS_FK", 'R', &["ID_PAIS"]);
    fk.ref_table = Some("PAIS".to_string());
    fk.ref_owner = Some("MAESTROS".to_string());
    fk.ref_columns = vec!["ID_PAIS".to_string()];
    assert!(
        fk.write(&table, &settings(false))
            .contains("REFERENCES maestros.pais ("),
        "{}",
        fk.write(&table, &settings(false))
    );

    let mut local = constraint("CLIENTE_PAIS_FK", 'R', &["ID_PAIS"]);
    local.ref_table = Some("PAIS".to_string());
    local.ref_owner = Some("ANAMNESIS".to_string());
    local.ref_columns = vec!["ID_PAIS".to_string()];
    assert!(local
        .write(&table, &settings(false))
        .contains("REFERENCES pais ("));
}

#[test]
fn trigger_header_replaces_only_the_table_token() {
    // A table called NEW must not rewrite `REFERENCING NEW AS NEW OLD AS OLD`.
    let trigger = Trigger {
        owner: "APP".to_string(),
        name: "T".to_string(),
        table_owner: "APP".to_string(),
        table_name: "NEW".to_string(),
        description: "T \nBEFORE INSERT ON NEW \nREFERENCING NEW AS NEW OLD AS OLD\nFOR EACH ROW\n"
            .to_string(),
        trigger_body: "BEGIN NULL; END;".to_string(),
        when_clause: None,
        action_type: "PL/SQL".to_string(),
    };
    let output = trigger.write(&settings(false));
    assert!(output.contains("BEFORE INSERT ON new\n"), "{output}");
    assert!(
        output.contains("REFERENCING NEW AS NEW OLD AS OLD\n"),
        "{output}"
    );
}

#[test]
fn primary_key_prints_using_index_storage() {
    let mut key = constraint("PK_CLIENTE", 'P', &["ID_CLIENTE"]);
    key.using_index = Some(sqlt_meta::IndexStorage {
        tablespace: Some("CONTABILIDAD_DATA".to_string()),
        next_extent: Some(1024 * 1024),
    });
    assert_eq!(
        key.write(&table(), &settings(true)),
        "ALTER TABLE cliente\n  ADD CONSTRAINT pk_cliente PRIMARY KEY (\n    id_cliente\n  )\n  USING INDEX\n    TABLESPACE contabilidad_data\n    STORAGE (\n      NEXT       1024 K\n    )\n/\n\n"
    );
    // `NoStorageForConstraint` drops the whole block.
    let without_storage = WriteSettings {
        no_storage_for_constraint: true,
        ..settings(true)
    };
    assert_eq!(
        key.write(&table(), &without_storage),
        "ALTER TABLE cliente\n  ADD CONSTRAINT pk_cliente PRIMARY KEY (\n    id_cliente\n  )\n/\n\n"
    );
    // Foreign keys never carry index storage.
    let mut fk = constraint("CLIENTE_PAIS_FK", 'R', &["ID_PAIS"]);
    fk.ref_table = Some("PAIS".to_string());
    fk.ref_columns = vec!["ID_PAIS".to_string()];
    fk.using_index = key.using_index.clone();
    assert!(!fk.write(&table(), &settings(true)).contains("USING INDEX"));
}

#[test]
fn index_prints_its_storage() {
    let mut table = Table::new("ANAMNESIS", "CLIENTE");
    table.columns = vec![column("CI", "VARCHAR2", 50, 0, 0, true, "")];
    let index = Index {
        name: "IDX_CLIENTE_CI".to_string(),
        unique: true,
        bitmap: false,
        columns: vec!["CI".to_string()],
        tablespace: Some("CONTABILIDAD_DATA".to_string()),
        next_extent: Some(1024 * 1024),
        compress: None,
    };
    assert_eq!(
        index.write(&table, &settings(true)),
        "CREATE UNIQUE INDEX idx_cliente_ci\n  ON cliente (\n    ci\n  )\n  TABLESPACE contabilidad_data\n  STORAGE (\n    NEXT       1024 K\n  )\n/\n\n"
    );
}

#[test]
fn prompts_match_the_windows_output() {
    let mut table = table();
    table.indexes.push(Index {
        name: "IDX_CLIENTE_CI".to_string(),
        unique: true,
        bitmap: false,
        columns: vec!["CI".to_string()],
        tablespace: None,
        next_extent: None,
        compress: None,
    });
    table
        .constraints
        .push(constraint("PK_CLIENTE", 'P', &["ID_CLIENTE"]));
    table.triggers.push(Trigger {
        owner: "ANAMNESIS".to_string(),
        name: "CLIENTE_BT".to_string(),
        table_owner: "ANAMNESIS".to_string(),
        table_name: "CLIENTE".to_string(),
        description: "CLIENTE_BT \nBEFORE INSERT ON CLIENTE \nFOR EACH ROW\n".to_string(),
        trigger_body: "BEGIN\n  NULL;\nEND;".to_string(),
        when_clause: None,
        action_type: "PL/SQL".to_string(),
    });
    let mut options = settings(false);
    options.generate_prompts = true;
    let output = table.write_full(&options);
    assert!(
        output.starts_with("PROMPT CREATE TABLE cliente\nCREATE TABLE cliente ("),
        "{output}"
    );
    assert!(
        output.contains("PROMPT CREATE INDEX idx_cliente_ci\nCREATE UNIQUE INDEX idx_cliente_ci"),
        "{output}"
    );
    assert!(
        output.contains(
            "PROMPT ALTER TABLE cliente ADD CONSTRAINT pk_cliente PRIMARY KEY\nALTER TABLE cliente"
        ),
        "{output}"
    );
    assert!(
        output.contains(
            "PROMPT CREATE OR REPLACE TRIGGER cliente_bt\nCREATE OR REPLACE TRIGGER cliente_bt"
        ),
        "{output}"
    );
}

#[test]
fn transform_script_matches_the_helper_layout() {
    let mut table = table();
    table.comments = Some("Tabla de clientes".to_string());
    table.columns[1].comments = Some("Cédula".to_string());
    table.indexes.push(Index {
        name: "IDX_CLIENTE_CI".to_string(),
        unique: true,
        bitmap: false,
        columns: vec!["CI".to_string()],
        tablespace: None,
        next_extent: None,
        compress: None,
    });
    table
        .constraints
        .push(constraint("PK_CLIENTE", 'P', &["ID_CLIENTE"]));
    table.triggers.push(Trigger {
        owner: "ANAMNESIS".to_string(),
        name: "CLIENTE_BT".to_string(),
        table_owner: "ANAMNESIS".to_string(),
        table_name: "CLIENTE".to_string(),
        description: "CLIENTE_BT \nBEFORE INSERT ON CLIENTE \nFOR EACH ROW\n".to_string(),
        trigger_body: "BEGIN\n  NULL;\nEND;".to_string(),
        when_clause: None,
        action_type: "PL/SQL".to_string(),
    });
    table.grants.push(Grant {
        grantee: "REPORTE".to_string(),
        privileges: vec!["SELECT".to_string()],
        grantable: false,
        column_privileges: Vec::new(),
    });
    let mut options = settings(true);
    options.generate_prompts = true;
    let script = sqlt_meta::transform_script(&table, "CLIENTE_$00", &[], &options);

    assert!(
        script.contains("LOCK TABLE cliente IN EXCLUSIVE MODE NOWAIT\n/\n\n"),
        "{script}"
    );
    assert!(script.contains("RENAME cliente TO cliente_$00\n/\n\n\n"));
    assert!(script.contains("DROP TRIGGER cliente_bt\n/\n\n"));
    assert!(script.contains("ALTER TABLE cliente_$00\n  DROP CONSTRAINT pk_cliente CASCADE\n/\n\n"));
    assert!(script.contains("DROP INDEX idx_cliente_ci\n/\n\n"));
    assert!(script
        .contains("PROMPT CREATE cliente\nPROMPT CREATE TABLE cliente\nCREATE TABLE cliente ("));
    assert!(script.contains("INSERT /*+append*/ INTO cliente (\n  id_cliente,\n"));
    assert!(script.contains("  FROM cliente_$00\n)\n/\n\n\n"));
    assert!(script.contains(
        "PROMPT CREATE indexes\nPROMPT CREATE INDEX idx_cliente_ci\nCREATE UNIQUE INDEX idx_cliente_ci"
    ));
    assert!(script.contains(
        "PROMPT CREATE constraints\nPROMPT ALTER TABLE cliente ADD CONSTRAINT pk_cliente PRIMARY KEY\n"
    ));
    assert!(
        script.contains("PROMPT CREATE triggers\nPROMPT CREATE OR REPLACE TRIGGER cliente_bt\n")
    );
    assert!(script
        .contains("PROMPT CREATE comments\nCOMMENT ON TABLE cliente IS 'Tabla de clientes';\n"));
    assert!(script.contains("PROMPT GRANT privileges\nGRANT SELECT ON cliente TO reporte;\n"));
}

#[test]
fn range_partitions_match_metobjects_layout() {
    use sqlt_meta::{Partition, PartitionKind, Partitioning};

    let mut table = Table::new("APP", "PEDIDOS");
    table.columns = vec![
        column("ID", "NUMBER", 22, 10, 0, false, ""),
        column("FECHA", "DATE", 7, 0, 0, true, ""),
    ];
    table.partitioning = Some(Partitioning {
        kind: PartitionKind::Range,
        columns: vec!["FECHA".to_string()],
        sub_kind: None,
        sub_columns: Vec::new(),
        sub_templates: Vec::new(),
        interval: None,
        partitions: vec![
            Partition {
                name: "P_2018".to_string(),
                high_value: Some("TO_DATE('2019-01-01', 'YYYY-MM-DD')".to_string()),
                tablespace: None,
                next_extent: None,
                subpartitions: Vec::new(),
            },
            Partition {
                name: "P_MAX".to_string(),
                high_value: Some("MAXVALUE".to_string()),
                tablespace: Some("DATA".to_string()),
                next_extent: Some(1024 * 1024),
                subpartitions: Vec::new(),
            },
        ],
        row_movement: true,
    });
    let expected = "CREATE TABLE pedidos (\n  id    NUMBER(10,0) NOT NULL,\n  fecha DATE         NULL\n)\n  PARTITION BY RANGE (\n    fecha\n  )\n  (\n    PARTITION p_2018 VALUES LESS THAN (TO_DATE('2019-01-01', 'YYYY-MM-DD')),\n    PARTITION p_max  VALUES LESS THAN (MAXVALUE)\n      TABLESPACE data\n      STORAGE (\n        NEXT       1024 K\n      )\n  )\n  ENABLE ROW MOVEMENT\n/\n\n";
    assert_eq!(table.write_definition(&settings(false)), expected);
}

#[test]
fn hash_partitions_omit_values() {
    use sqlt_meta::{Partition, PartitionKind, Partitioning};

    let mut table = Table::new("APP", "HASHED");
    table.columns = vec![column("ID", "NUMBER", 22, 10, 0, false, "")];
    table.partitioning = Some(Partitioning {
        kind: PartitionKind::Hash,
        columns: vec!["ID".to_string()],
        sub_kind: None,
        sub_columns: Vec::new(),
        sub_templates: Vec::new(),
        interval: None,
        partitions: vec![
            Partition {
                name: "P1".to_string(),
                high_value: None,
                tablespace: None,
                next_extent: None,
                subpartitions: Vec::new(),
            },
            Partition {
                name: "P2".to_string(),
                high_value: None,
                tablespace: None,
                next_extent: None,
                subpartitions: Vec::new(),
            },
        ],
        row_movement: false,
    });
    let ddl = table.write_definition(&settings(false));
    assert!(
        ddl.contains("  PARTITION BY HASH (\n    id\n  )\n"),
        "{ddl}"
    );
    assert!(ddl.contains("    PARTITION p1,\n"), "{ddl}");
    assert!(!ddl.contains("VALUES"), "{ddl}");
    assert!(ddl.ends_with("  )\n/\n\n"), "{ddl}");
}

#[test]
fn list_partitions_use_single_key() {
    use sqlt_meta::{Partition, PartitionKind, Partitioning};

    let mut table = Table::new("APP", "REGIONES");
    table.columns = vec![column("REGION", "CHAR", 1, 0, 0, false, "")];
    table.partitioning = Some(Partitioning {
        kind: PartitionKind::List,
        columns: vec!["REGION".to_string(), "IGNORED".to_string()],
        sub_kind: None,
        sub_columns: Vec::new(),
        sub_templates: Vec::new(),
        interval: None,
        partitions: vec![Partition {
            name: "P_NORTE".to_string(),
            high_value: Some("'N'".to_string()),
            tablespace: None,
            next_extent: None,
            subpartitions: Vec::new(),
        }],
        row_movement: false,
    });
    let ddl = table.write_definition(&settings(false));
    assert!(ddl.contains("  PARTITION BY LIST (region)\n"), "{ddl}");
    assert!(
        ddl.contains("    PARTITION p_norte VALUES ('N')\n"),
        "{ddl}"
    );
}

#[test]
fn composite_partitions_use_template() {
    use sqlt_meta::{Partition, PartitionKind, Partitioning, Subpartition};

    let mut table = Table::new("APP", "VENTAS");
    table.columns = vec![
        column("ID", "NUMBER", 22, 10, 0, false, ""),
        column("REGION", "CHAR", 1, 0, 0, false, ""),
        column("FECHA", "DATE", 7, 0, 0, false, ""),
    ];
    let templates = vec![
        Subpartition {
            name: "SP_N".to_string(),
            high_value: Some("'N'".to_string()),
            tablespace: None,
        },
        Subpartition {
            name: "SP_S".to_string(),
            high_value: Some("'S'".to_string()),
            tablespace: None,
        },
    ];
    table.partitioning = Some(Partitioning {
        kind: PartitionKind::Range,
        columns: vec!["FECHA".to_string()],
        sub_kind: Some(PartitionKind::List),
        sub_columns: vec!["REGION".to_string()],
        sub_templates: templates.clone(),
        interval: None,
        partitions: vec![
            Partition {
                name: "P1".to_string(),
                high_value: Some("TO_DATE('2020-01-01', 'YYYY-MM-DD')".to_string()),
                tablespace: None,
                next_extent: None,
                subpartitions: templates.clone(),
            },
            Partition {
                name: "P2".to_string(),
                high_value: Some("TO_DATE('2021-01-01', 'YYYY-MM-DD')".to_string()),
                tablespace: None,
                next_extent: None,
                subpartitions: vec![
                    Subpartition {
                        name: "SP_N".to_string(),
                        high_value: Some("'N'".to_string()),
                        tablespace: Some("OTHER".to_string()),
                    },
                    Subpartition {
                        name: "SP_S".to_string(),
                        high_value: Some("'S'".to_string()),
                        tablespace: Some("OTHER".to_string()),
                    },
                ],
            },
        ],
        row_movement: false,
    });
    let expected = "CREATE TABLE ventas (\n  id     NUMBER(10,0) NOT NULL,\n  region CHAR(1)      NOT NULL,\n  fecha  DATE         NOT NULL\n)\n  PARTITION BY RANGE (\n    fecha\n  )\n  SUBPARTITION BY LIST (region)\n  SUBPARTITION TEMPLATE\n  (\n    SUBPARTITION sp_n VALUES ('N'),\n    SUBPARTITION sp_s VALUES ('S')\n  )\n  (\n    PARTITION p1 VALUES LESS THAN (TO_DATE('2020-01-01', 'YYYY-MM-DD')),\n    PARTITION p2 VALUES LESS THAN (TO_DATE('2021-01-01', 'YYYY-MM-DD'))\n      (\n        SUBPARTITION sp_n VALUES ('N') TABLESPACE other,\n        SUBPARTITION sp_s VALUES ('S') TABLESPACE other\n      )\n  )\n/\n\n";
    assert_eq!(table.write_definition(&settings(false)), expected);
}

#[test]
fn iot_with_overflow_matches_metobjects_layout() {
    use sqlt_meta::IotInfo;

    let mut table = Table::new("APP", "EMP_IOT");
    table.columns = vec![
        column("ID", "NUMBER", 22, 10, 0, false, ""),
        column("NAME", "VARCHAR2", 50, 0, 0, true, ""),
        column("SALARY", "NUMBER", 22, 10, 2, true, ""),
    ];
    table
        .constraints
        .push(constraint("PK_EMP_IOT", 'P', &["ID"]));
    table.iot = Some(IotInfo {
        pct_threshold: Some(20),
        include_column: Some(2),
        overflow_tablespace: Some("OVFL".to_string()),
        overflow_next_extent: Some(2048 * 1024),
        partitioning: None,
    });
    table.tablespace = Some("DATA".to_string());
    table.next_extent = Some(1024 * 1024);
    let expected = "CREATE TABLE emp_iot (\n  id     NUMBER(10,0) NOT NULL,\n  name   VARCHAR2(50) NULL,\n  salary NUMBER(10,2) NULL,\n  CONSTRAINT pk_emp_iot PRIMARY KEY (\n    id\n  )\n)\n  ORGANIZATION INDEX INCLUDING name\n    TABLESPACE data\n    STORAGE (\n      NEXT       1024 K\n    )\n  PCTTHRESHOLD  20\n  OVERFLOW\n    TABLESPACE ovfl\n    STORAGE (\n      NEXT       2048 K\n    )\n/\n\n";
    assert_eq!(table.write_definition(&settings(true)), expected);
}

#[test]
fn partitioned_iot_uses_index_partitions() {
    use sqlt_meta::{IotInfo, IotPartition, IotPartitioning, PartitionKind};

    let mut table = Table::new("APP", "EVENT_IOT");
    table.columns = vec![
        column("ID", "NUMBER", 22, 10, 0, false, ""),
        column("EVENT_DATE", "DATE", 7, 0, 0, false, ""),
        column("PAYLOAD", "VARCHAR2", 100, 0, 0, true, ""),
    ];
    table
        .constraints
        .push(constraint("PK_EVENT_IOT", 'P', &["ID", "EVENT_DATE"]));
    table.iot = Some(IotInfo {
        pct_threshold: None,
        include_column: None,
        overflow_tablespace: None,
        overflow_next_extent: None,
        partitioning: Some(IotPartitioning {
            kind: PartitionKind::Range,
            columns: vec!["EVENT_DATE".to_string()],
            partitions: vec![
                IotPartition {
                    name: "P1".to_string(),
                    high_value: Some("TO_DATE('2020-01-01', 'YYYY-MM-DD')".to_string()),
                    tablespace: None,
                    next_extent: None,
                },
                IotPartition {
                    name: "P2".to_string(),
                    high_value: Some("MAXVALUE".to_string()),
                    tablespace: Some("DATA".to_string()),
                    next_extent: Some(1024 * 1024),
                },
            ],
        }),
    });
    table.tablespace = Some("DATA".to_string());
    table.next_extent = Some(1024 * 1024);
    let ddl = table.write_definition(&settings(true));
    assert!(
        ddl.contains("  PARTITION BY RANGE (\n    event_date\n  )\n  (\n"),
        "{ddl}"
    );
    assert!(
        ddl.contains("    PARTITION p1 VALUES LESS THAN (TO_DATE('2020-01-01', 'YYYY-MM-DD')),\n"),
        "{ddl}"
    );
    assert!(
        ddl.contains("    PARTITION p2 VALUES LESS THAN (MAXVALUE)\n      STORAGE ("),
        "{ddl}"
    );
    let organization = ddl.find("ORGANIZATION INDEX").expect("iot clause");
    let partitions = ddl.find("PARTITION BY RANGE").expect("partition clause");
    assert!(organization < partitions, "{ddl}");
}

#[test]
fn temporary_table_clause() {
    let mut table = Table::new("APP", "TMP_ROWS");
    table.columns = vec![column("A", "NUMBER", 22, 10, 0, true, "")];
    table.temporary = Some(sqlt_meta::TemporaryDuration::Preserve);
    let expected = "CREATE GLOBAL TEMPORARY TABLE tmp_rows (\n  a NUMBER(10,0) NULL\n)\n  ON COMMIT PRESERVE ROWS\n/\n\n";
    assert_eq!(table.write_definition(&settings(false)), expected);
    table.temporary = Some(sqlt_meta::TemporaryDuration::Delete);
    assert!(table
        .write_definition(&settings(false))
        .contains("  ON COMMIT DELETE ROWS\n"));
}

#[test]
fn cache_clause() {
    let mut table = Table::new("APP", "CACHED");
    table.columns = vec![column("A", "NUMBER", 22, 10, 0, true, "")];
    table.cache = true;
    let expected = "CREATE TABLE cached (\n  a NUMBER(10,0) NULL\n)\n  CACHE\n/\n\n";
    assert_eq!(table.write_definition(&settings(false)), expected);
}

#[test]
fn virtual_column_uses_as_expression() {
    let mut table = Table::new("APP", "EMP");
    table.columns = vec![
        column("SALARIO", "NUMBER", 22, 10, 2, true, ""),
        column("DOBLE", "NUMBER", 22, 10, 2, true, "salario * 2"),
    ];
    table.columns[1].virtual_column = true;
    let ddl = table.write_definition(&settings(false));
    assert!(
        ddl.contains("  doble   NUMBER(10,2) AS (salario * 2) NULL\n"),
        "{ddl}"
    );
}
