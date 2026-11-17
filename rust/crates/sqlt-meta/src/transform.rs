// SPDX-License-Identifier: GPL-3.0-only
//! "Table Transformation Helper": the script `CTableTransformer` generates
//! (`TableTransformer.cpp:262-500`) to rebuild a table in place — lock, rename to
//! a free `_$NN` name, drop dependants, create, copy the data and recreate
//! everything.

use crate::{Constraint, Table, WriteSettings};

const HEADER: &[&str] = &[
    "PROMPT ================================================================",
    "PROMPT        <<< SQLTools Table Transformation Helper V1.1 >>>        ",
    "PROMPT    CAUTION:                                                     ",
    "PROMPT      No warranty of any kind is expressed or implied. You       ",
    "PROMPT      use at your own risk. The author will not be liable for    ",
    "PROMPT      data loss, damages, loss of profits or any other kind      ",
    "PROMPT      of loss while using or misusing this software.             ",
    "PROMPT    USAGE:                                                       ",
    "PROMPT      You must modify and execute each statement step by step.   ",
    "PROMPT ================================================================",
];

fn column_list(columns: &[String], indent: usize, lower: bool) -> String {
    let pad = " ".repeat(indent);
    let mut out = String::new();
    for (index, column) in columns.iter().enumerate() {
        out.push_str(&pad);
        out.push_str(&Table::display_name(column, lower));
        if index + 1 < columns.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out
}

pub fn transform_script(
    table: &Table,
    backup: &str,
    dependent_foreign_keys: &[(Table, Constraint)],
    settings: &WriteSettings,
) -> String {
    let lower = settings.lower_db_name;
    let name = Table::display_name(&table.name, lower);
    let backup_name = Table::display_name(backup, lower);
    let mut out = String::new();

    for line in HEADER {
        out.push_str(line);
        out.push('\n');
    }
    out.push('\n');

    out.push_str("PROMPT LOCK table in exclusive mode\n");
    out.push_str("PROMPT   The following lock does not prevent from accessing the table while it's being rebult.\n");
    out.push_str(
        "PROMPT   It helps to make sure that the table is not in use at this particular moment\n",
    );
    out.push_str("PROMPT   and NOWAIT helps to do it w/o waiting. Hello TOAD developers!\n");
    out.push_str("PROMPT If ERROR then STOP and REPEAT before SUCCESS!\n");
    out.push_str(&format!(
        "LOCK TABLE {name} IN EXCLUSIVE MODE NOWAIT\n/\n\n"
    ));

    out.push_str("PROMPT RENAME table\n");
    out.push_str(&format!("RENAME {name} TO {backup_name}\n/\n\n\n"));

    out.push_str("PROMPT DROP triggers\n");
    for trigger in &table.triggers {
        out.push_str(&format!("{}\n/\n\n", trigger.drop_statement(settings)));
    }
    out.push('\n');

    out.push_str("PROMPT DROP primary, unique and named constraints\n");
    for constraint in &table.constraints {
        if constraint.kind == 'P' || constraint.kind == 'U' || !constraint.generated {
            out.push_str(&format!(
                "ALTER TABLE {backup_name}\n  DROP CONSTRAINT {}",
                Table::display_name(&constraint.name, lower)
            ));
            if constraint.kind == 'P' || constraint.kind == 'U' {
                out.push_str(" CASCADE");
            }
            out.push_str("\n/\n\n");
        }
    }
    out.push('\n');

    out.push_str("PROMPT DROP indexes\n");
    for index in &table.indexes {
        out.push_str(&format!(
            "DROP INDEX {}\n/\n\n",
            Table::display_name(&index.name, lower)
        ));
    }
    out.push('\n');

    out.push_str(&format!("PROMPT CREATE {name}\n"));
    out.push_str(&table.write_definition(settings));
    out.push('\n');

    out.push_str("PROMPT TRANSFER data into new table\n");
    let columns: Vec<String> = table.columns.iter().map(|c| c.name.clone()).collect();
    out.push_str(&format!("INSERT /*+append*/ INTO {name} (\n"));
    out.push_str(&column_list(&columns, 2, lower));
    out.push_str(")\n(\n");
    out.push_str("  SELECT\n");
    out.push_str(&column_list(&columns, 4, lower));
    out.push_str(&format!("  FROM {backup_name}\n"));
    out.push_str(")\n/\n\n\n");

    out.push_str("PROMPT CREATE indexes\n");
    for index in &table.indexes {
        out.push_str(&index.write(table, settings));
    }
    out.push('\n');

    out.push_str("PROMPT CREATE constraints\n");
    for kind in ['C', 'P', 'U', 'R'] {
        for constraint in table
            .constraints
            .iter()
            .filter(|c| c.kind == kind && !c.is_not_null_check())
        {
            out.push_str(&constraint.write(table, settings));
        }
    }
    out.push('\n');

    out.push_str("PROMPT CREATE dependent foreign key constraints\n");
    for (referencing_table, constraint) in dependent_foreign_keys {
        out.push_str(&constraint.write(referencing_table, settings));
    }
    out.push('\n');

    out.push_str("PROMPT CREATE triggers\n");
    for trigger in &table.triggers {
        out.push_str(&trigger.write(settings));
    }
    out.push('\n');

    out.push_str("PROMPT CREATE comments\n");
    out.push_str(&table.write_comments(settings));
    out.push('\n');

    out.push_str("PROMPT GRANT privileges\n");
    for grant in &table.grants {
        out.push_str(&grant.write(table, settings));
    }
    out
}
