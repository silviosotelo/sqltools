#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use sqlt_app::{catalog, AppState, ConnectionConfig, HistoryEntry};
use sqlt_core::{Batch, ColumnMeta};
use tauri::ipc::Channel;
use tauri::State;

#[derive(Clone, serde::Serialize)]
#[serde(tag = "type", content = "data", rename_all = "lowercase")]
enum QueryEvent {
    Columns(Vec<ColumnMeta>),
    Batch(Batch),
    Done {
        #[serde(rename = "rows")]
        rows: usize,
        #[serde(rename = "elapsedMs")]
        elapsed_ms: u64,
    },
    Error(String),
}

#[tauri::command]
fn list_connections(state: State<'_, Arc<AppState>>) -> Result<Vec<ConnectionConfig>, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "Connection store lock poisoned".to_string())?;
    Ok(store.list())
}

#[tauri::command]
fn save_connection(
    state: State<'_, Arc<AppState>>,
    config: ConnectionConfig,
    password: Option<String>,
) -> Result<(), String> {
    let save_password = config.save_password;
    let id = config.id.clone();
    {
        let mut store = state
            .store
            .lock()
            .map_err(|_| "Connection store lock poisoned".to_string())?;
        store.upsert(config).map_err(|e| e.to_string())?;
        if let Some(password) = password.filter(|_| save_password) {
            store
                .set_password(&id, &password)
                .map_err(|e| e.to_string())?;
        } else if !save_password {
            let _ = store.delete_password(&id);
        }
    }
    Ok(())
}

#[tauri::command]
fn delete_connection(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "Connection store lock poisoned".to_string())?;
    store.remove(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn connect(state: State<'_, Arc<AppState>>, id: String) -> Result<String, String> {
    state.connect(&id)
}

#[tauri::command]
fn disconnect(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    state.disconnect(&id)
}

#[tauri::command]
fn execute(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
    sql: String,
    max_rows: usize,
    channel: Channel<QueryEvent>,
) -> Result<(), String> {
    let session = state.session(&connection_id)?;
    let options = state.query_options(&connection_id);
    let state = state.inner().clone();
    let started_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let outcome = (|| -> Result<usize, String> {
            let mut handle = session.query(&sql, options).map_err(|e| e.to_string())?;
            channel
                .send(QueryEvent::Columns(handle.columns().to_vec()))
                .ok();
            let mut total = 0usize;
            loop {
                let want = if max_rows == 0 {
                    500
                } else {
                    (max_rows - total).min(500)
                };
                if want == 0 {
                    break;
                }
                let batch = handle.next_batch(want).map_err(|e| e.to_string())?;
                if batch.rows.is_empty() {
                    break;
                }
                total += batch.rows.len();
                channel.send(QueryEvent::Batch(batch)).ok();
            }
            Ok(total)
        })();
        let elapsed_ms = started.elapsed().as_millis() as u64;
        let rows = outcome.as_ref().copied().unwrap_or(0);
        let error = outcome.as_ref().err().cloned();
        state.record(HistoryEntry {
            connection_id,
            sql,
            rows,
            elapsed_ms,
            error: error.clone(),
            started_at,
        });
        match outcome {
            Ok(rows) => {
                channel.send(QueryEvent::Done { rows, elapsed_ms }).ok();
            }
            Err(message) => {
                channel.send(QueryEvent::Error(message)).ok();
            }
        }
    });
    Ok(())
}

#[tauri::command]
fn cancel(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    state.session(&id)?.cancel().map_err(|e| e.to_string())
}

#[tauri::command]
fn commit(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    state.session(&id)?.commit().map_err(|e| e.to_string())
}

#[tauri::command]
fn rollback(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    state.session(&id)?.rollback().map_err(|e| e.to_string())
}

#[tauri::command]
fn history(state: State<'_, Arc<AppState>>) -> Vec<HistoryEntry> {
    state.history()
}

#[tauri::command]
fn list_table_names(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
) -> Result<Vec<String>, String> {
    let session = state.session(&connection_id)?;
    catalog::list_table_names(&session, state.query_options(&connection_id))
}

#[tauri::command]
fn list_objects(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
    schema: Option<String>,
    object_type: Option<String>,
) -> Result<Vec<catalog::ObjectInfo>, String> {
    let session = state.session(&connection_id)?;
    catalog::list_objects(
        &session,
        schema.as_deref(),
        object_type.as_deref(),
        state.query_options(&connection_id),
    )
}

#[tauri::command]
fn list_columns(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
    schema: Option<String>,
    table: String,
) -> Result<Vec<catalog::ColumnInfo>, String> {
    let session = state.session(&connection_id)?;
    catalog::list_columns(
        &session,
        schema.as_deref(),
        &table,
        state.query_options(&connection_id),
    )
}

#[tauri::command]
fn load_ddl(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
    schema: Option<String>,
    name: String,
    object_type: String,
) -> Result<String, String> {
    let session = state.session(&connection_id)?;
    catalog::load_ddl(
        &session,
        schema.as_deref(),
        &name,
        &object_type,
        state.query_options(&connection_id),
    )
}

#[tauri::command]
fn session_stats(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
) -> Result<Vec<(String, String)>, String> {
    let session = state.session(&connection_id)?;
    catalog::session_stats(&session, state.query_options(&connection_id))
}

#[tauri::command]
fn explain_plan(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
    sql: String,
) -> Result<String, String> {
    let session = state.session(&connection_id)?;
    session
        .execute(&format!("explain plan for {sql}"))
        .map_err(|e| e.to_string())?;
    let mut query = session
        .query(
            "select plan_table_output from table(dbms_xplan.display())",
            state.query_options(&connection_id),
        )
        .map_err(|e| e.to_string())?;
    let mut lines = Vec::new();
    loop {
        let batch = query.next_batch(500).map_err(|e| e.to_string())?;
        if batch.rows.is_empty() {
            break;
        }
        for row in batch.rows {
            if let Some(line) = row.first().and_then(|value| value.display()) {
                lines.push(line.to_string());
            }
        }
    }
    Ok(lines.join("\n"))
}

fn main() {
    let state = AppState::new().unwrap_or_else(|error| {
        eprintln!("Could not load the connection store: {error}");
        std::process::exit(1);
    });
    tauri::Builder::default()
        .manage(Arc::new(state))
        .invoke_handler(tauri::generate_handler![
            list_connections,
            save_connection,
            delete_connection,
            connect,
            disconnect,
            execute,
            cancel,
            commit,
            rollback,
            history,
            list_table_names,
            list_objects,
            list_columns,
            load_ddl,
            session_stats,
            explain_plan
        ])
        .run(tauri::generate_context!())
        .expect("error while running SQLTools");
}
