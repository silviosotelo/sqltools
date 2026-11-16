// SPDX-License-Identifier: GPL-3.0-only
//! End-to-end check of the application state: JSON store + OS keyring + Oracle.
//! Requires SQLT_USER, SQLT_PASS, SQLT_ALIAS, SQLT_TNS_ADMIN and SQLT_OCI_DIR.

use std::fs;

use sqlt_app::{AppState, ConnectionConfig, ConnectionStore};

#[test]
#[ignore = "requires OS keyring and a real Oracle connection"]
fn connect_and_query_through_app_state() {
    let user = std::env::var("SQLT_USER").expect("SQLT_USER");
    let password = std::env::var("SQLT_PASS").expect("SQLT_PASS");
    let alias = std::env::var("SQLT_ALIAS").expect("SQLT_ALIAS");
    let tns_admin = std::env::var("SQLT_TNS_ADMIN").ok();
    let oci_dir = std::env::var("SQLT_OCI_DIR").ok();

    let path =
        std::env::temp_dir().join(format!("sqlt-app-integration-{}.json", std::process::id()));
    let _ = fs::remove_file(&path);
    let mut store = ConnectionStore::new(path.clone());
    let mut config = ConnectionConfig::new("Integration", &user);
    config.alias = Some(alias);
    config.tns_admin = tns_admin;
    config.oci_dir = oci_dir;
    store.upsert(config.clone()).unwrap();
    store.set_password(&config.id, &password).unwrap();

    let state = AppState::with_store(store);
    let version = state.connect(&config.id).expect("connect");
    assert!(version.contains("Oracle"), "{version}");

    let session = state.session(&config.id).expect("session");
    let mut query = session
        .query("select 1 from dual", state.query_options(&config.id))
        .expect("query");
    let batch = query.next_batch(1).expect("fetch");
    assert_eq!(batch.rows.len(), 1);

    state.disconnect(&config.id).unwrap();
    let store = state.store.lock().unwrap();
    let _ = store.delete_password(&config.id);
    drop(store);
    let _ = fs::remove_file(path);
}

#[test]
#[ignore = "requires OS keyring and a real Oracle connection"]
fn catalog_and_ddl() {
    use sqlt_app::catalog;

    let user = std::env::var("SQLT_USER").expect("SQLT_USER");
    let password = std::env::var("SQLT_PASS").expect("SQLT_PASS");
    let alias = std::env::var("SQLT_ALIAS").expect("SQLT_ALIAS");
    let tns_admin = std::env::var("SQLT_TNS_ADMIN").ok();
    let oci_dir = std::env::var("SQLT_OCI_DIR").ok();

    let path = std::env::temp_dir().join(format!("sqlt-app-catalog-{}.json", std::process::id()));
    let _ = fs::remove_file(&path);
    let mut store = ConnectionStore::new(path.clone());
    let mut config = ConnectionConfig::new("Catalog", &user);
    config.alias = Some(alias);
    config.tns_admin = tns_admin;
    config.oci_dir = oci_dir;
    store.upsert(config.clone()).unwrap();
    store.set_password(&config.id, &password).unwrap();

    let state = AppState::with_store(store);
    state.connect(&config.id).expect("connect");
    let session = state.session(&config.id).expect("session");
    let options = state.query_options(&config.id);

    let tables = catalog::list_table_names(&session, options).expect("table names");
    assert!(!tables.is_empty(), "the schema has no tables/views");
    let first = tables[0].clone();

    let columns = catalog::list_columns(&session, None, &first, options).expect("columns");
    assert!(!columns.is_empty(), "table {first} has no columns");
    eprintln!(
        "catalog: {} tables, {} columns in {first}",
        tables.len(),
        columns.len()
    );

    let meta_ddl = catalog::load_table_ddl_meta(&session, None, &first, options).expect("meta ddl");

    eprintln!("meta ddl: {} bytes", meta_ddl.len());
    let ddl = catalog::load_ddl(&session, None, &first, "TABLE", options).expect("ddl");
    assert!(ddl.to_ascii_uppercase().contains("CREATE"), "{ddl}");

    let objects = catalog::list_objects(&session, None, Some("TABLE"), options).expect("objects");
    assert!(objects.iter().any(|object| object.name == first));

    let constraints =
        catalog::list_constraints(&session, None, &first, options).expect("constraints");
    let indexes = catalog::list_indexes(&session, None, &first, options).expect("indexes");
    let triggers = catalog::list_triggers(&session, None, &first, options).expect("triggers");
    let grants = catalog::list_grants(&session, None, &first, options).expect("grants");
    eprintln!(
        "details {first}: {} constraints, {} indexes, {} triggers, {} grants",
        constraints.len(),
        indexes.len(),
        triggers.len(),
        grants.len()
    );
    match catalog::session_stats(&session, options) {
        Ok(stats) => {
            assert!(!stats.is_empty());
            eprintln!("session stats: {} entries", stats.len());
        }
        Err(error) => eprintln!("session stats unavailable: {error}"),
    }

    state.disconnect(&config.id).unwrap();
    let store = state.store.lock().unwrap();
    let _ = store.delete_password(&config.id);
    drop(store);
    let _ = fs::remove_file(path);
}
