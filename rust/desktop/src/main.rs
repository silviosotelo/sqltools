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
    Output(Vec<String>),
    StatementError {
        message: String,
        offset: usize,
    },
    Done {
        #[serde(rename = "rows")]
        rows: usize,
        #[serde(rename = "elapsedMs")]
        elapsed_ms: u64,
        errors: usize,
    },
    Error(String),
}

fn query_event(event: sqlt_app::script::ScriptEvent) -> QueryEvent {
    use sqlt_app::script::ScriptEvent;
    match event {
        ScriptEvent::Columns(columns) => QueryEvent::Columns(columns),
        ScriptEvent::Batch(batch) => QueryEvent::Batch(batch),
        ScriptEvent::Output(lines) => QueryEvent::Output(lines),
        ScriptEvent::StatementError { message, offset } => {
            QueryEvent::StatementError { message, offset }
        }
    }
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
async fn connect(state: State<'_, Arc<AppState>>, id: String) -> Result<String, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.connect(&id))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn disconnect(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.disconnect(&id))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn enable_dbms_output(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
) -> Result<(), String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let session = state.session(&connection_id)?;
        // Wait for a running script: the buffer belongs to the whole session.
        let _guard = session.lock_execution();
        session.enable_dbms_output().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn execute(
    state: State<'_, Arc<AppState>>,
    connection_id: String,
    sql: String,
    max_rows: usize,
    offset: Option<usize>,
    halt_on_error: bool,
    channel: Channel<QueryEvent>,
) -> Result<(), String> {
    use sqlt_app::script::{run_script, run_script_continue};

    let session = state.session(&connection_id)?;
    let options = state.query_options(&connection_id);
    let state = state.inner().clone();
    let started_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || {
        use sqlt_app::script::ScriptEvent;
        let started = Instant::now();
        // Keep the first continuable failure so history does not show a script
        // with failed statements as a clean success.
        let mut first_error: Option<String> = None;
        let emit = |event| {
            if let ScriptEvent::StatementError { message, .. } = &event {
                if first_error.is_none() {
                    first_error = Some(message.clone());
                }
            }
            channel.send(query_event(event)).ok();
        };
        let outcome = if halt_on_error {
            run_script(&session, &sql, options, max_rows, offset, emit)
        } else {
            run_script_continue(&session, &sql, options, max_rows, offset, emit)
        };
        let elapsed_ms = started.elapsed().as_millis() as u64;
        let rows = outcome.as_ref().map(|script| script.last_rows).unwrap_or(0);
        let error = outcome.as_ref().err().cloned().or(first_error);
        state.record(HistoryEntry {
            connection_id,
            sql,
            rows,
            elapsed_ms,
            error: error.clone(),
            started_at,
        });
        match outcome {
            Ok(script) => {
                channel
                    .send(QueryEvent::Done {
                        rows: script.last_rows,
                        elapsed_ms,
                        errors: script.errors,
                    })
                    .ok();
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
fn toggle_read_only(state: State<'_, Arc<AppState>>, id: String) -> Result<bool, String> {
    state.toggle_read_only(&id)
}

#[tauri::command]
async fn commit(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let session = state.session(&id)?;
        // Commit waits for the running statement so it cannot commit halfway
        // through a script.
        let _guard = session.lock_execution();
        session.commit().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn rollback(state: State<'_, Arc<AppState>>, id: String) -> Result<(), String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let session = state.session(&id)?;
        let _guard = session.lock_execution();
        session.rollback().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
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
fn list_templates() -> Result<Vec<String>, String> {
    sqlt_app::templates::list_templates().map_err(|e| e.to_string())
}

#[tauri::command]
fn load_template(name: String) -> Result<Option<sqlt_app::templates::Template>, String> {
    sqlt_app::templates::load_template(&name).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_template(name: String, body: String) -> Result<(), String> {
    sqlt_app::templates::save_template(&name, &body).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_template(name: String) -> Result<(), String> {
    sqlt_app::templates::delete_template(&name).map_err(|e| e.to_string())
}

#[tauri::command]
fn read_sql_file(path: String) -> Result<sqlt_app::fileio::FileText, String> {
    sqlt_app::fileio::read_sql_file(&path)
}

#[tauri::command]
fn write_sql_file(path: String, text: String, eol: String) -> Result<(), String> {
    sqlt_app::fileio::write_sql_file(&path, &text, &eol)
}

fn dialog_path(path: tauri_plugin_dialog::FilePath) -> Option<String> {
    path.into_path().ok().map(|path| path.display().to_string())
}

#[tauri::command]
async fn pick_sql_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (sender, mut receiver) = tauri::async_runtime::channel(1);
    app.dialog()
        .file()
        .add_filter("SQL", &["sql", "pls", "pks", "pkb", "txt"])
        .pick_file(move |path| {
            let _ = sender.try_send(path);
        });
    let chosen = receiver
        .recv()
        .await
        .ok_or_else(|| "File dialog closed unexpectedly".to_string())?;
    Ok(chosen.and_then(dialog_path))
}

#[tauri::command]
async fn pick_sql_save_path(
    app: tauri::AppHandle,
    suggested: Option<String>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (sender, mut receiver) = tauri::async_runtime::channel(1);
    let mut dialog = app.dialog().file().add_filter("SQL", &["sql", "txt"]);
    if let Some(name) = suggested.as_deref() {
        let file_name = name.rsplit(['\\', '/']).next().unwrap_or(name);
        dialog = dialog.set_file_name(file_name);
    }
    dialog.save_file(move |path| {
        let _ = sender.try_send(path);
    });
    let chosen = receiver
        .recv()
        .await
        .ok_or_else(|| "File dialog closed unexpectedly".to_string())?;
    Ok(chosen.and_then(dialog_path))
}

#[tauri::command]
fn save_workspace(workspace: sqlt_app::Workspace) -> Result<(), String> {
    sqlt_app::workspace::save_default(&workspace).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_workspace_to(path: String, workspace: sqlt_app::Workspace) -> Result<(), String> {
    sqlt_app::workspace::save_to(std::path::Path::new(&path), &workspace).map_err(|e| e.to_string())
}

#[tauri::command]
async fn pick_workspace_save_path(
    app: tauri::AppHandle,
    suggested: Option<String>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (sender, mut receiver) = tauri::async_runtime::channel(1);
    let mut dialog = app.dialog().file().add_filter("Workspace", &["json"]);
    let name = suggested
        .as_deref()
        .and_then(|value| value.rsplit(['\\', '/']).next())
        .filter(|value| !value.is_empty())
        .unwrap_or("workspace.json");
    dialog = dialog.set_file_name(name);
    dialog.save_file(move |path| {
        let _ = sender.try_send(path);
    });
    let chosen = receiver
        .recv()
        .await
        .ok_or_else(|| "File dialog closed unexpectedly".to_string())?;
    Ok(chosen.and_then(dialog_path))
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
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

const SQL_HELP_URL: &str = "https://docs.oracle.com/en/database/oracle/oracle-database/12.2/sqlrf/";

#[cfg(target_os = "windows")]
fn reveal_target(path: &str) -> (String, Vec<String>) {
    ("explorer".to_string(), vec![format!("/select,{path}")])
}

#[cfg(target_os = "macos")]
fn reveal_target(path: &str) -> (String, Vec<String>) {
    ("open".to_string(), vec!["-R".to_string(), path.to_string()])
}

#[cfg(all(unix, not(target_os = "macos")))]
fn reveal_target(path: &str) -> (String, Vec<String>) {
    let parent = std::path::Path::new(path)
        .parent()
        .map(|dir| dir.display().to_string())
        .unwrap_or_else(|| path.to_string());
    ("xdg-open".to_string(), vec![parent])
}

#[cfg(target_os = "windows")]
fn browser_target(url: &str) -> (String, Vec<String>) {
    (
        "rundll32".to_string(),
        vec!["url.dll,FileProtocolHandler".to_string(), url.to_string()],
    )
}

#[cfg(target_os = "macos")]
fn browser_target(url: &str) -> (String, Vec<String>) {
    ("open".to_string(), vec![url.to_string()])
}

#[cfg(all(unix, not(target_os = "macos")))]
fn browser_target(url: &str) -> (String, Vec<String>) {
    ("xdg-open".to_string(), vec![url.to_string()])
}

fn spawn_target(target: (String, Vec<String>)) -> Result<(), String> {
    let mut child = std::process::Command::new(target.0)
        .args(target.1)
        .spawn()
        .map_err(|e| e.to_string())?;
    // Reap the launcher so it does not stay as a zombie on Unix.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[tauri::command]
fn reveal_file_location(path: String) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("No file is open".to_string());
    }
    spawn_target(reveal_target(&path))
}

#[tauri::command]
fn open_sql_help() -> Result<(), String> {
    spawn_target(browser_target(SQL_HELP_URL))
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateInfo {
    version: String,
    current_version: String,
    notes: Option<String>,
    date: Option<String>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
}

#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<Option<UpdateInfo>, String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => Ok(Some(UpdateInfo {
            version: update.version.clone(),
            current_version: update.current_version.clone(),
            notes: update.body.clone(),
            date: update.date.map(|date| date.to_string()),
        })),
        Ok(None) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command]
async fn install_update(
    app: tauri::AppHandle,
    channel: tauri::ipc::Channel<UpdateProgress>,
) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "No update is available".to_string())?;
    update
        .download_and_install(
            move |downloaded, total| {
                let _ = channel.send(UpdateProgress {
                    downloaded: downloaded as u64,
                    total,
                });
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    let state = AppState::new().unwrap_or_else(|error| {
        eprintln!("Could not load the connection store: {error}");
        std::process::exit(1);
    });
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(Arc::new(state))
        .invoke_handler(tauri::generate_handler![
            list_connections,
            save_connection,
            delete_connection,
            connect,
            disconnect,
            enable_dbms_output,
            toggle_read_only,
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
            list_templates,
            load_template,
            save_template,
            delete_template,
            read_sql_file,
            write_sql_file,
            pick_sql_file,
            pick_sql_save_path,
            save_workspace,
            save_workspace_to,
            pick_workspace_save_path,
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
            check_update,
            install_update,
            exit_app,
            reveal_file_location,
            open_sql_help,
            explain_plan
        ])
        .run(tauri::generate_context!())
        .expect("error while running SQLTools");
}
