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
