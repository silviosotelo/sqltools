// Temporary live check against the real database. It reuses the connection saved by the app (the password
// comes from the OS keyring through ConnectionStore), so no credential is read, printed or written here.
//   cargo run -p sqlt-app --example live_check
// Delete after use; the permanent versions live in tests/app_integration.rs behind #[ignore].
use sqlt_app::{catalog, AppState, ConnectionStore};

fn main() {
    let store = ConnectionStore::load().expect("connection store");
    let config = store
        .list()
        .into_iter()
        .find(|config| config.alias.as_deref() == Some("desa02"))
        .or_else(|| store.list().into_iter().next())
        .expect("a saved connection");
    println!(
        "using saved connection label={} user={} alias={:?} (password from the OS keyring)",
        config.label, config.user, config.alias
    );

    let state = AppState::with_store(store);
    let banner = state.connect(&config.id).expect("connect");
    println!("connected: {banner}");
    let session = state.session(&config.id).expect("session");
    let options = state.query_options(&config.id);

    // 1) count_objects == list_objects for every type of the explorer.
    let counts = catalog::count_objects(&session, None, options).expect("count_objects");
    let mut mismatches = Vec::new();
    println!("-- count_objects vs list_objects");
    for kind in [
        "TABLE",
        "VIEW",
        "SEQUENCE",
        "PROCEDURE",
        "FUNCTION",
        "PACKAGE",
        "TRIGGER",
        "TYPE",
        "SYNONYM",
    ] {
        let listed = catalog::list_objects(&session, None, Some(kind), options)
            .expect("list_objects")
            .len() as u64;
        let counted = counts
            .iter()
            .find(|count| count.object_type == kind)
            .map_or(0, |count| count.count);
        println!("{kind:<10} count={counted:<6} list={listed}");
        if counted != listed {
            mismatches.push(kind);
        }
    }
    let hidden: Vec<_> = counts
        .iter()
        .filter(|count| ["PACKAGE BODY", "INDEX", "LOB"].contains(&count.object_type.as_str()))
        .map(|count| format!("{}={}", count.object_type, count.count))
        .collect();
    println!("types not shown by the explorer: {hidden:?}");
    assert!(mismatches.is_empty(), "counts differ for {mismatches:?}");

    // 2) transaction_pending follows update/commit/rollback (FOR UPDATE opens a transaction, no data touched).
    let pending = || catalog::transaction_pending(&session, options).expect("transaction_pending");
    let open = || {
        session
            .query_with("select 1 from dual for update", &[], options)
            .expect("select for update")
            .next_batch(1)
            .expect("fetch");
    };
    println!("-- transaction_pending");
    assert!(!pending(), "no transaction at the start");
    open();
    assert!(pending(), "after FOR UPDATE");
    println!("FOR UPDATE -> true");
    session.commit().expect("commit");
    assert!(!pending(), "after commit");
    println!("commit      -> false");
    open();
    assert!(pending(), "after FOR UPDATE again");
    session.rollback().expect("rollback");
    assert!(!pending(), "after rollback");
    println!("rollback    -> false");

    state.disconnect(&config.id).ok();
    println!("LIVE CHECK OK");
}
