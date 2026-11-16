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
fn list_constraints(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
    schema: Option<String>,
    table: String,
) -> Result<Vec<catalog::ConstraintInfo>, String> {
    let session = state.session(&connection_id)?;
    catalog::list_constraints(
        &session,
        schema.as_deref(),
        &table,
        state.query_options(&connection_id),
    )
}

#[tauri::command]
fn list_indexes(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
    schema: Option<String>,
    table: String,
) -> Result<Vec<catalog::IndexInfo>, String> {
    let session = state.session(&connection_id)?;
    catalog::list_indexes(
        &session,
        schema.as_deref(),
        &table,
        state.query_options(&connection_id),
    )
}

#[tauri::command]
fn list_triggers(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
    schema: Option<String>,
    table: String,
) -> Result<Vec<catalog::TriggerInfo>, String> {
    let session = state.session(&connection_id)?;
    catalog::list_triggers(
        &session,
        schema.as_deref(),
        &table,
        state.query_options(&connection_id),
    )
}

#[tauri::command]
fn list_grants(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
    schema: Option<String>,
    table: String,
) -> Result<Vec<catalog::GrantInfo>, String> {
    let session = state.session(&connection_id)?;
    catalog::list_grants(
        &session,
        schema.as_deref(),
        &table,
        state.query_options(&connection_id),
    )
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

#[tauri::command]
fn load_ddl_meta(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
    schema: Option<String>,
    name: String,
) -> Result<String, String> {
    let session = state.session(&connection_id)?;
    catalog::load_table_ddl_meta(
        &session,
        schema.as_deref(),
        &name,
        state.query_options(&connection_id),
    )
}

#[tauri::command]
fn list_commands() -> Vec<sqlt_app::CommandSpec> {
    sqlt_app::registry()
}

#[tauri::command]
fn save_workspace(workspace: sqlt_app::Workspace) -> Result<(), String> {
    sqlt_app::workspace::save_default(&workspace).map_err(|e| e.to_string())
}

#[tauri::command]
fn load_workspace() -> Result<sqlt_app::Workspace, String> {
    sqlt_app::workspace::load_default().map_err(|e| e.to_string())
}

#[tauri::command]
fn migrate_legacy_connections(
    state: State<'_, Arc<AppState>>,
    path: String,
) -> Result<usize, String> {
    let mut store = state
        .store
        .lock()
        .map_err(|_| "Connection store lock poisoned".to_string())?;
    sqlt_app::migrate::import_legacy_json(std::path::Path::new(&path), &mut store)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn export_csv(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
    sql: String,
    max_rows: usize,
    path: Option<String>,
) -> Result<String, String> {
    let session = state.session(&connection_id)?;
    let target = match path.map(|value| value.trim().to_string()) {
        Some(value) if !value.is_empty() => std::path::PathBuf::from(value),
        _ => std::env::temp_dir().join(format!(
            "sqltools-export-{}.csv",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or_default()
        )),
    };
    sqlt_app::export::export_csv(
        &session,
        &sql,
        state.query_options(&connection_id),
        &target,
        max_rows,
    )?;
    Ok(target.display().to_string())
}

#[tauri::command]
fn format_sql(sql: String, uppercase: bool) -> String {
    use sqlt_core::format::{FormatOptions, KeywordCase};
    let options = FormatOptions {
        keyword_case: if uppercase {
            KeywordCase::Upper
        } else {
            KeywordCase::Keep
        },
        ..FormatOptions::default()
    };
    sqlt_core::format::format_sql(&sql, &options)
}

#[tauri::command]
fn format_sql_with(sql: String, options: sqlt_core::format::FormatOptions) -> String {
    sqlt_core::format::format_sql(&sql, &options)
}

#[tauri::command]
fn infer_format_profile(sql: String) -> sqlt_core::format::FormatOptions {
    sqlt_core::format::infer_from_example(&sql)
}

#[tauri::command]
fn list_format_profiles() -> Result<Vec<String>, String> {
    sqlt_app::profiles::profile_names().map_err(|e| e.to_string())
}

#[tauri::command]
fn save_format_profile(
    name: String,
    options: sqlt_core::format::FormatOptions,
) -> Result<(), String> {
    sqlt_app::profiles::save_profile(&name, &options).map_err(|e| e.to_string())
}

#[tauri::command]
fn load_format_profile(name: String) -> Result<sqlt_core::format::FormatOptions, String> {
    sqlt_app::profiles::load_profile(&name).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_format_profile(name: String) -> Result<(), String> {
    sqlt_app::profiles::delete_profile(&name).map_err(|e| e.to_string())
}

#[tauri::command]
async fn check_updates(app: tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => Ok(format!("Update available: {}", update.version)),
        Ok(None) => Ok("No updates available".to_string()),
        Err(error) => Err(error.to_string()),
    }
}

fn main() {
    let state = AppState::new().unwrap_or_else(|error| {
        eprintln!("Could not load the connection store: {error}");
        std::process::exit(1);
    });
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
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
            load_ddl_meta,
            session_stats,
            list_constraints,
            list_indexes,
            list_triggers,
            list_grants,
            list_commands,
            save_workspace,
            load_workspace,
            migrate_legacy_connections,
            export_csv,
            format_sql,
            format_sql_with,
            infer_format_profile,
            list_format_profiles,
            save_format_profile,
            load_format_profile,
            delete_format_profile,
            check_updates,
            explain_plan
        ])
        .run(tauri::generate_context!())
        .expect("error while running SQLTools");
}
