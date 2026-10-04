#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod catalog;
mod desktop;
mod scheduler;
mod tray;

use drive_core::{
    atomic_json,
    cli::{validate_remote, Cli},
    model::*,
    now,
    operations::{OperationQueue, QueueView, Spec, Status, CANCELLED},
    sync::synchronize,
    update, Result,
};
use serde::Serialize;
use std::{
    fs,
    path::PathBuf,
    sync::{atomic::Ordering, Arc, Mutex},
};
use tauri::{Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;

const MAIN_TRAY_ID: &str = "main-tray";

struct AppState {
    config: Mutex<Config>,
    catalog: drive_core::catalog::Catalog,
    runtime: Mutex<Runtime>,
    queue: OperationQueue,
    data: PathBuf,
    cli: Cli,
    computers: Cli,
    tray_navigation: Mutex<Option<&'static str>>,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Runtime {
    connected: bool,
    busy: bool,
    operation: String,
    current_pair: Option<String>,
    current_file: Option<String>,
    cli_version: String,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct View {
    config: Config,
    runtime: Runtime,
    data_path: String,
    autostart: bool,
    tray_available: bool,
    computer: Option<ComputerRegistration>,
    queue: QueueView,
}

impl AppState {
    fn set_sync_paused(&self, paused: bool) -> Result<()> {
        {
            let mut config = self.config.lock().unwrap();
            let mut next = config.clone();
            next.paused = paused;
            atomic_json(&self.data.join("settings.json"), &next)?;
            *config = next;
        }
        if paused {
            self.queue.cancel_syncs(true);
        }
        Ok(())
    }
    fn save(&self) -> Result<()> {
        atomic_json(
            &self.data.join("settings.json"),
            &*self.config.lock().unwrap(),
        )
    }
    fn activity(
        &self,
        app: &tauri::AppHandle,
        kind: &str,
        message: impl Into<String>,
        pair_id: Option<String>,
    ) {
        let event = Activity {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: now(),
            kind: kind.into(),
            message: message.into(),
            pair_id,
        };
        {
            let mut c = self.config.lock().unwrap();
            c.events.insert(0, event.clone());
            c.events.truncate(200);
        }
        let _ = self.save();
        let _ = app.emit("drive-activity", event);
        let _ = app.emit("drive-changed", ());
    }
    async fn connect(&self) -> Result<()> {
        let result = self.cli.account().await;
        match result {
            Ok((id, email)) => {
                let mut config = self.config.lock().unwrap();
                if config
                    .account_id
                    .as_ref()
                    .is_some_and(|previous| previous != &id)
                    && !config.pairs.is_empty()
                {
                    self.catalog.deactivate();
                    self.runtime.lock().unwrap().connected = false;
                    return Err("This account differs from the one associated with your folders. Remove pairings before switching accounts.".into());
                }
                self.catalog.activate(&id);
                config.account_id = Some(id);
                config.account_email = email;
                drop(config);
                self.save()?;
                let mut runtime = self.runtime.lock().unwrap();
                runtime.connected = true;
                runtime.error = None;
                Ok(())
            }
            Err(error) => {
                self.catalog.deactivate();
                let mut r = self.runtime.lock().unwrap();
                r.connected = false;
                r.error = Some(error.clone());
                Err(error)
            }
        }
    }
}

#[tauri::command]
fn get_state(app: tauri::AppHandle, state: State<'_, Arc<AppState>>) -> View {
    let queue = state.queue.snapshot();
    let mut runtime = state.runtime.lock().unwrap().clone();
    let current = queue
        .items
        .iter()
        .filter(|item| matches!(item.status, Status::Running | Status::Cancelling))
        .min_by_key(|item| match item.lane {
            drive_core::operations::Lane::Transfer => 0,
            drive_core::operations::Lane::Exclusive => 1,
            drive_core::operations::Lane::Interactive => 2,
            drive_core::operations::Lane::Background => 3,
        });
    runtime.busy = current.is_some();
    runtime.operation = current.map(|item| item.kind.clone()).unwrap_or_default();
    runtime.current_pair = current.and_then(|item| item.pair_id.clone());
    runtime.current_file = current.map(|item| item.detail.clone());
    let config = state.config.lock().unwrap().clone();
    let computer = config
        .account_id
        .as_ref()
        .and_then(|id| config.computer_registrations.get(id))
        .cloned();
    View {
        config,
        computer,
        runtime,
        queue,
        data_path: state.data.to_string_lossy().into(),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        tray_available: app.tray_by_id(MAIN_TRAY_ID).is_some(),
    }
}

#[tauri::command]
async fn refresh_connection(app: tauri::AppHandle, state: State<'_, Arc<AppState>>) -> Result<()> {
    let queue = state.queue.clone();
    queue
        .execute(
            Spec::new("connection", "Check connection", "Proton account").interactive(),
            |_operation| async move {
                let result = state.connect().await;
                let _ = app.emit("drive-changed", ());
                result
            },
        )
        .await
}

#[tauri::command]
async fn login(app: tauri::AppHandle, state: State<'_, Arc<AppState>>) -> Result<()> {
    let queue = state.queue.clone();
    queue
        .execute(
            Spec::new("login", "Connect Proton account", "Browser authentication").exclusive(),
            |_operation| async move {
                let result = async {
                    state.cli.run(&["auth", "login", "--json"], 600).await?;
                    state.connect().await
                }
                .await;
                match &result {
                    Ok(_) => {
                        state.activity(&app, "success", "Proton account connected", None);
                        catalog::preload(&app, state.inner());
                    }
                    Err(e) => state.activity(&app, "error", e.clone(), None),
                }
                result
            },
        )
        .await
}

#[tauri::command]
async fn logout(app: tauri::AppHandle, state: State<'_, Arc<AppState>>) -> Result<()> {
    let queue = state.queue.clone();
    queue
        .execute(
            Spec::new("logout", "Disconnect account", "Sign out and pause sync").exclusive(),
            |_operation| async move {
                state.cli.run(&["auth", "logout", "--json"], 60).await?;
                state.runtime.lock().unwrap().connected = false;
                state.catalog.deactivate();
                state.queue.cancel_waiting_account_operations();
                state.config.lock().unwrap().paused = true;
                state.save()?;
                state.activity(&app, "info", "Account disconnected; sync paused", None);
                Ok(())
            },
        )
        .await
}

#[tauri::command]
async fn list_remote(state: State<'_, Arc<AppState>>, path: String) -> Result<Vec<RemoteEntry>> {
    let queue = state.queue.clone();
    queue
        .execute(
            Spec::new(
                "browse",
                "List folders",
                drive_core::i18n::raw(path.clone()),
            )
            .interactive(),
            |_operation| async move { state.cli.folders(&path).await },
        )
        .await
}

#[tauri::command]
async fn create_remote_folder(
    state: State<'_, Arc<AppState>>,
    parent: String,
    name: String,
) -> Result<()> {
    let queue = state.queue.clone();
    queue
        .execute(
            Spec::new(
                "folder",
                drive_core::i18n::message(
                    "Create folder {0}",
                    &[serde_json::json!(name.to_string())],
                ),
                drive_core::i18n::raw(parent.clone()),
            ),
            |_operation| async move {
                validate_remote(&parent)?;
                drive_core::cli::validate_name(&name)?;
                state
                    .cli
                    .run(
                        &["filesystem", "create-folder", &parent, &name, "--json"],
                        120,
                    )
                    .await?;
                state.catalog.invalidate();
                Ok(())
            },
        )
        .await
}

#[tauri::command]
async fn save_pair(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    mut pair: SyncPair,
    computer_root: Option<String>,
) -> Result<()> {
    let queue = state.queue.clone();
    queue
        .execute(
            Spec {
                pair_id: (!pair.id.is_empty()).then(|| pair.id.clone()),
                ..Spec::new(
                    "configure",
                    drive_core::i18n::message(
                        "Configure {0}",
                        &[serde_json::json!(pair.name.to_string())],
                    ),
                    drive_core::i18n::raw(pair.local_path.clone()),
                )
            },
            |_operation| async move {
                if let Some(root) = &computer_root {
                    validate_remote(root)?;
                    if !pair.id.is_empty()
                        || !root.starts_with("/devices/")
                        || root.split('/').count() != 3
                    {
                        return Err("Select a computer for the new folder.".into());
                    }
                    drive_core::cli::validate_name(&pair.name)?;
                    pair.remote_path = format!("{root}/{}", pair.name);
                }
                drive_core::cli::validate_source(&pair.remote_path)?;
                let photos =
                    pair.remote_path == "/photos" || pair.remote_path.starts_with("/albums");
                if photos && pair.propagate_deletions {
                    return Err("Photos and albums preserve deletions.".into());
                }
                if pair.remote_path == "/albums" && pair.mode != SyncMode::Download {
                    return Err("All albums use Drive → PC mode.".into());
                }
                if pair.name.trim().is_empty() || pair.name.len() > 120 {
                    return Err("Choose a name up to 120 characters.".into());
                }
                if !(1..=1440).contains(&pair.interval_minutes) {
                    return Err("Choose an interval between 1 and 1440 minutes.".into());
                }
                let local = fs::canonicalize(&pair.local_path)
                    .map_err(|_| "Select an existing local folder.")?;
                if !local.is_dir() || local.parent().is_none() {
                    return Err("Select a folder, not the system root.".into());
                }
                pair.local_path = local.to_str().ok_or("Path is not UTF-8.")?.into();
                {
                    let c = state.config.lock().unwrap();
                    for other in &c.pairs {
                        if other.id == pair.id {
                            continue;
                        }
                        let other_local = PathBuf::from(&other.local_path);
                        let remote_overlap = pair.remote_path == other.remote_path
                            || pair
                                .remote_path
                                .starts_with(&format!("{}/", other.remote_path))
                            || other
                                .remote_path
                                .starts_with(&format!("{}/", pair.remote_path));
                        if local.starts_with(&other_local)
                            || other_local.starts_with(&local)
                            || remote_overlap
                        {
                            return Err(
                                "This folder overlaps another pairing. Choose independent folders."
                                    .into(),
                            );
                        }
                    }
                    if !pair.id.is_empty() {
                        let old = c
                            .pairs
                            .iter()
                            .find(|p| p.id == pair.id)
                            .ok_or("Pairing not found.")?;
                        if old.local_path != pair.local_path || old.remote_path != pair.remote_path
                        {
                            return Err(
                                "To change paths, remove this pairing and create another.".into()
                            );
                        }
                        pair.device_uid = old.device_uid.clone();
                    }
                }
                state.connect().await?;
                if let Some(device) = state
                    .cli
                    .computer_for_path(&pair.remote_path)
                    .await
                    .or_else(|e| if photos { Ok(None) } else { Err(e) })?
                {
                    if !pair.id.is_empty() && pair.device_uid.as_deref() != Some(&device.uid) {
                        return Err(
                            "The computer's identity changed. Reconfigure the pairing.".into()
                        );
                    }
                    pair.device_uid = Some(device.uid);
                } else {
                    pair.device_uid = None;
                }
                if let Some(root) = &computer_root {
                    let existing = state.cli.list(root).await?;
                    match existing.iter().find(|e| e.name == pair.name) {
                        Some(entry) if !entry.directory => {
                            return Err(
                                "A file with this name already exists on the computer.".into()
                            )
                        }
                        Some(_) => (),
                        None => {
                            state
                                .cli
                                .run(
                                    &["filesystem", "create-folder", root, &pair.name, "--json"],
                                    120,
                                )
                                .await?;
                        }
                    }
                }
                if pair.remote_path == "/photos" {
                    state.cli.photos(None).await?;
                } else if pair.remote_path == "/albums" {
                    state.cli.albums().await?;
                } else if pair.remote_path.starts_with("/albums/") {
                    state.cli.photos(Some(&pair.remote_path)).await?;
                } else {
                    state.cli.folders(&pair.remote_path).await?;
                }
                {
                    let mut c = state.config.lock().unwrap();
                    if pair.id.is_empty() {
                        pair.id = uuid::Uuid::new_v4().to_string();
                        pair.last_run = None;
                        c.pairs.push(pair.clone());
                    } else {
                        let old = c
                            .pairs
                            .iter_mut()
                            .find(|p| p.id == pair.id)
                            .ok_or("Pairing not found.")?;
                        if old.local_path != pair.local_path || old.remote_path != pair.remote_path
                        {
                            return Err(
                                "To change paths, remove this pairing and create another.".into()
                            );
                        }
                        pair.last_run = old.last_run;
                        *old = pair.clone();
                    }
                }
                state.save()?;
                state.activity(
                    &app,
                    "info",
                    drive_core::i18n::message(
                        "Folder {0} configured",
                        &[serde_json::json!(pair.name.to_string())],
                    ),
                    Some(pair.id),
                );
                Ok(())
            },
        )
        .await
}

#[tauri::command]
async fn remove_pair(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: String,
) -> Result<()> {
    let queue = state.queue.clone();
    queue
        .execute(
            Spec {
                pair_id: Some(id.clone()),
                ..Spec::new("configure", "Remove pairing", "Keep files on both sides")
            },
            |_operation| async move {
                {
                    let mut c = state.config.lock().unwrap();
                    c.pairs.retain(|p| p.id != id);
                }
                state.save()?;
                let _ = app.emit("drive-changed", ());
                Ok(())
            },
        )
        .await
}

#[tauri::command]
fn set_preferences(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    paused: bool,
    auto_update: bool,
    autostart: bool,
    close_to_tray: Option<bool>,
) -> Result<()> {
    if close_to_tray == Some(true) && app.tray_by_id(MAIN_TRAY_ID).is_none() {
        return Err("The system tray is unavailable in this session.".into());
    }
    if app.autolaunch().is_enabled().map_err(|e| e.to_string())? != autostart {
        if autostart {
            app.autolaunch().enable().map_err(|e| e.to_string())?;
        } else {
            app.autolaunch().disable().map_err(|e| e.to_string())?;
        }
    }
    {
        let mut c = state.config.lock().unwrap();
        let mut next = c.clone();
        next.paused = paused;
        next.auto_update = auto_update;
        if let Some(enabled) = close_to_tray {
            next.close_to_tray = enabled;
        }
        // Closing behavior only changes once the preference is safely persisted.
        atomic_json(&state.data.join("settings.json"), &next)?;
        *c = next;
    }
    if paused {
        state.queue.cancel_syncs(true);
    }
    let _ = app.emit("drive-changed", ());
    Ok(())
}

#[tauri::command]
fn set_locale(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    locale: Locale,
) -> Result<()> {
    // Persist before publishing, keeping other preferences and pending jobs intact.
    {
        let mut config = state.config.lock().unwrap();
        let mut next = config.clone();
        next.locale = locale;
        atomic_json(&state.data.join("settings.json"), &next)?;
        *config = next;
    }
    let _ = app.emit("drive-changed", ());
    Ok(())
}

#[tauri::command]
fn take_tray_navigation(state: State<'_, Arc<AppState>>) -> Option<&'static str> {
    state.tray_navigation.lock().unwrap().take()
}

async fn run_pairs(
    app: &tauri::AppHandle,
    state: &Arc<AppState>,
    ids: Option<Vec<String>>,
    automatic: bool,
) -> Result<()> {
    let queue = state.queue.clone();
    let spec = {
        let config = state.config.lock().unwrap();
        let selected: Vec<_> = config
            .pairs
            .iter()
            .filter(|p| p.enabled && ids.as_ref().is_none_or(|ids| ids.contains(&p.id)))
            .collect();
        let title = if selected.len() == 1 {
            drive_core::i18n::message(
                "Sync {0}",
                &[serde_json::json!(selected[0].name.to_string())],
            )
        } else {
            drive_core::i18n::message("Sync {0} folders", &[serde_json::json!(selected.len())])
        };
        Spec {
            automatic,
            can_cancel_running: true,
            pair_id: (selected.len() == 1).then(|| selected[0].id.clone()),
            ..Spec::new(
                "sync",
                title,
                if automatic {
                    "Automatic sync"
                } else {
                    "Requested by you"
                },
            )
        }
    };
    queue
        .execute(spec, |operation| {
            execute_pairs(app, state, ids, automatic, operation)
        })
        .await
}

async fn execute_pairs(
    app: &tauri::AppHandle,
    state: &Arc<AppState>,
    ids: Option<Vec<String>>,
    automatic: bool,
    operation: drive_core::operations::Context,
) -> Result<()> {
    if operation.cancel.load(Ordering::Relaxed) {
        return Err(CANCELLED.into());
    }
    if automatic && state.config.lock().unwrap().paused {
        return Err(CANCELLED.into());
    }
    state.connect().await?;
    if operation.cancel.load(Ordering::Relaxed)
        || (automatic && state.config.lock().unwrap().paused)
    {
        return Err(CANCELLED.into());
    }
    let pairs = state.config.lock().unwrap().pairs.clone();
    let mut first_error = None;
    for pair in pairs
        .into_iter()
        .filter(|p| p.enabled && ids.as_ref().is_none_or(|ids| ids.contains(&p.id)))
    {
        if operation.cancel.load(Ordering::Relaxed) {
            first_error = Some(CANCELLED.into());
            break;
        }
        operation.progress(
            Some(pair.id.clone()),
            drive_core::i18n::message("Comparing {0}", &[serde_json::json!(pair.name.to_string())]),
        );
        state.activity(
            app,
            "info",
            drive_core::i18n::message("Syncing {0}", &[serde_json::json!(pair.name.to_string())]),
            Some(pair.id.clone()),
        );
        let result = synchronize(
            &state.cli,
            &pair,
            &state
                .data
                .join("snapshots")
                .join(format!("{}.json", pair.id)),
            &operation.cancel,
            |kind, message| {
                let detail = match kind {
                    "upload" => drive_core::i18n::message(
                        "Uploading · {0}",
                        &[serde_json::json!(message.to_string())],
                    ),
                    "download" => drive_core::i18n::message(
                        "Downloading · {0}",
                        &[serde_json::json!(message.to_string())],
                    ),
                    _ => message.to_owned(),
                };
                operation.progress(Some(pair.id.clone()), detail);
                if matches!(kind, "conflict" | "trash" | "recovery") {
                    state.activity(app, kind, message, Some(pair.id.clone()));
                } else {
                    let _ = app.emit("drive-changed", ());
                }
            },
        )
        .await;
        {
            let mut c = state.config.lock().unwrap();
            if let Some(p) = c.pairs.iter_mut().find(|p| p.id == pair.id) {
                p.last_run = Some(now());
            }
        }
        match result {
            Ok(report) => state.activity(
                app,
                if report.conflicts.is_empty() {
                    "success"
                } else {
                    "conflict"
                },
                drive_core::i18n::message("{0}: {1} uploaded, {2} downloaded, {3} unchanged, {4} trashed, {5} online documents, {6} conflicts", &[serde_json::json!(pair.name.to_string()), serde_json::json!(report.uploaded), serde_json::json!(report.downloaded), serde_json::json!(report.unchanged), serde_json::json!(report.deleted), serde_json::json!(report.online_only), serde_json::json!(report.conflicts.len())]),
                Some(pair.id),
            ),
            Err(e) => {
                state.activity(app, if operation.cancel.load(Ordering::Relaxed) { "info" } else { "error" }, drive_core::i18n::message("{0}: {1}", &[serde_json::json!(pair.name.to_string()), drive_core::i18n::nested(e.to_string())]), Some(pair.id));
                if first_error.is_none() {
                    first_error = Some(e);
                }
            }
        }
    }
    state.save()?;
    state.catalog.invalidate();
    first_error.map_or(Ok(()), Err)
}

#[tauri::command]
async fn sync_now(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    id: Option<String>,
) -> Result<()> {
    run_pairs(&app, state.inner(), id.map(|v| vec![v]), false).await
}

#[tauri::command]
fn cancel_operation(state: State<'_, Arc<AppState>>, id: String) -> Result<()> {
    state.queue.cancel(&id)
}

#[tauri::command]
fn pause_operation_queue(state: State<'_, Arc<AppState>>, paused: bool) {
    state.queue.pause(paused);
}

#[tauri::command]
fn clear_operation_history(state: State<'_, Arc<AppState>>) {
    state.queue.clear_finished();
}

#[tauri::command]
fn cancel_sync(state: State<'_, Arc<AppState>>) {
    state.queue.cancel_syncs(false);
}

async fn perform_update(
    app: &tauri::AppHandle,
    state: &Arc<AppState>,
    automatic: bool,
) -> Result<String> {
    let queue = state.queue.clone();
    queue
        .execute(
            Spec {
                automatic,
                ..Spec::new(
                    "update",
                    "Check for CLI updates",
                    "Download and SHA-512 verification, if a new version is available",
                )
                .exclusive()
            },
            |_operation| async move {
                if automatic && !state.config.lock().unwrap().auto_update {
                    return Err(CANCELLED.into());
                }
                let result = async {
                    let latest = update::latest().await?;
                    let current = state.runtime.lock().unwrap().cli_version.clone();
                    let current = semver::Version::parse(&current).ok();
                    let newer =
                        semver::Version::parse(&latest.version).map_err(|e| e.to_string())?;
                    let installed = current.as_ref().is_none_or(|current| &newer > current);
                    if installed {
                        update::install(&latest, &state.cli.binary).await?;
                        state.runtime.lock().unwrap().cli_version = latest.version.clone();
                    }
                    state.config.lock().unwrap().last_update_check = Some(now());
                    state.save()?;
                    let message = if installed {
                        drive_core::i18n::message(
                            "CLI updated to {0} · SHA-512 verified",
                            &[serde_json::json!(latest.version.to_string())],
                        )
                    } else {
                        drive_core::i18n::message(
                            "CLI {0} is up to date",
                            &[serde_json::json!(state
                                .runtime
                                .lock()
                                .unwrap()
                                .cli_version
                                .to_string())],
                        )
                    };
                    Ok::<_, String>(message)
                }
                .await;
                match &result {
                    Ok(message) => state.activity(app, "success", message, None),
                    Err(e) => {
                        state.config.lock().unwrap().last_update_check = Some(now());
                        state.activity(
                            app,
                            "error",
                            drive_core::i18n::message(
                                "Update: {0}",
                                &[drive_core::i18n::nested(e.to_string())],
                            ),
                            None,
                        );
                    }
                }
                result
            },
        )
        .await
}

#[tauri::command]
async fn update_cli(app: tauri::AppHandle, state: State<'_, Arc<AppState>>) -> Result<String> {
    perform_update(&app, state.inner(), false).await
}

#[tauri::command]
fn computer_name() -> String {
    fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty() && drive_core::cli::validate_name(s).is_ok())
        .unwrap_or_else(|| "My Linux PC".into())
}

#[tauri::command]
async fn register_computer(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    name: String,
    existing_uid: Option<String>,
) -> Result<RemoteEntry> {
    let queue = state.queue.clone();
    queue
        .execute(
            Spec::new(
                "computer",
                "Register computer",
                drive_core::i18n::raw(name.clone()),
            ),
            |_operation| async move {
                state.connect().await?;
                let (account, binding) = {
                    let c = state.config.lock().unwrap();
                    let account = c.account_id.clone().ok_or("Connect your Proton account.")?;
                    let binding = c.computer_registrations.get(&account).cloned();
                    (account, binding)
                };
                let devices = state.cli.computers().await?;
                let target = drive_core::computers::registration_target(
                    binding.as_ref(),
                    &devices,
                    &name,
                    existing_uid.as_deref(),
                )?;
                let save_binding = |registration: ComputerRegistration| -> Result<()> {
                    let mut c = state.config.lock().unwrap();
                    let mut next = c.clone();
                    next.computer_registrations
                        .insert(account.clone(), registration);
                    atomic_json(&state.data.join("settings.json"), &next)?;
                    *c = next;
                    let _ = app.emit("drive-changed", ());
                    Ok(())
                };
                let created = match target {
                    drive_core::computers::RegistrationTarget::Existing(device) => device,
                    drive_core::computers::RegistrationTarget::Create(name) => {
                        save_binding(ComputerRegistration {
                            name: name.clone(),
                            device_uid: None,
                        })?;
                        state.computers.register_computer(&name).await?
                    }
                };
                // Persist the returned identity before the independent CLI check:
                // even if its listing is delayed, another click reuses this UID.
                save_binding(ComputerRegistration {
                    name: created.name.clone(),
                    device_uid: Some(created.uid.clone()),
                })?;
                // Verify through the official CLI too: it is the executable used for all transfers.
                let device = state
        .cli
        .computers()
        .await?
        .into_iter()
        .find(|d| d.uid == created.uid)
        .ok_or("The CLI hasn't confirmed registration yet. Refresh the library and try again.")?;
                state.activity(
                    &app,
                    "success",
                    drive_core::i18n::message(
                        "{0} registered in Computers. Choose folders to sync.",
                        &[serde_json::json!(device.name.to_string())],
                    ),
                    None,
                );
                state.catalog.invalidate();
                Ok(device)
            },
        )
        .await
}

#[tauri::command]
fn list_library(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    path: String,
    force: Option<bool>,
    revision: Option<u64>,
) -> Result<drive_core::catalog::LibraryView> {
    catalog::request(
        &app,
        state.inner(),
        path,
        force.unwrap_or(false),
        revision,
        false,
    )
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Preview {
    kind: String,
    mime: String,
    content: String,
    name: String,
}

#[tauri::command]
async fn preview_file(state: State<'_, Arc<AppState>>, path: String) -> Result<Preview> {
    let queue = state.queue.clone();
    queue
        .execute(
            Spec::new(
                "preview",
                "Preview file",
                drive_core::i18n::raw(path.clone()),
            )
            .interactive(),
            |_operation| async move {
                use base64::Engine;
                if !state.runtime.lock().unwrap().connected {
                    return Err("Connect your Proton account before opening the library.".into());
                }
                let entry = drive_core::library::entry(&state.cli, &path).await?;
                if entry.native_document {
                    return Err("Open this document in the Proton editor.".into());
                }
                let mime = entry.media_type.as_str();
                let kind = if matches!(
                    mime,
                    "image/jpeg" | "image/png" | "image/webp" | "image/gif" | "image/avif"
                ) {
                    "image"
                } else if mime == "application/pdf" {
                    "pdf"
                } else if mime.starts_with("text/")
                    || matches!(mime, "application/json" | "application/xml")
                {
                    "text"
                } else {
                    "external"
                };
                if kind == "external" {
                    return Ok(Preview {
                        kind: kind.into(),
                        mime: mime.into(),
                        content: String::new(),
                        name: entry.name,
                    });
                }
                let limit = if kind == "text" {
                    2 * 1024 * 1024
                } else {
                    20 * 1024 * 1024
                };
                if entry.size > limit {
                    return Err(
            "This file is too large to preview. Use Download to open it on your computer."
                .into(),
        );
                }
                let stage = tempfile::tempdir().map_err(|e| e.to_string())?;
                let downloaded =
                    drive_core::library::download(&state.cli, &entry, stage.path()).await?;
                let bytes = fs::read(downloaded).map_err(|e| e.to_string())?;
                let content = if matches!(kind, "image" | "pdf") {
                    base64::engine::general_purpose::STANDARD.encode(bytes)
                } else {
                    String::from_utf8_lossy(&bytes).into_owned()
                };
                Ok(Preview {
                    kind: kind.into(),
                    mime: mime.into(),
                    content,
                    name: entry.name,
                })
            },
        )
        .await
}

#[tauri::command]
async fn download_file(
    state: State<'_, Arc<AppState>>,
    path: String,
    destination: String,
) -> Result<String> {
    let queue = state.queue.clone();
    queue
        .execute(
            Spec::new(
                "download",
                "Download file",
                drive_core::i18n::raw(format!("{path} → {destination}")),
            ),
            |_operation| async move {
                use std::io::Write;
                state.connect().await?;
                let entry = drive_core::library::entry(&state.cli, &path).await?;
                let root =
                    fs::canonicalize(&destination).map_err(|_| "Select an existing folder.")?;
                if !root.is_dir() {
                    return Err("Invalid destination.".into());
                }
                let target = drive_core::sync::safe_join(&root, &entry.name)?;
                let stage = tempfile::tempdir_in(&root).map_err(|e| e.to_string())?;
                let downloaded =
                    drive_core::library::download(&state.cli, &entry, stage.path()).await?;
                // persist_noclobber creates the final name atomically, never overwriting.
                let mut final_file =
                    tempfile::NamedTempFile::new_in(&root).map_err(|e| e.to_string())?;
                std::io::copy(
                    &mut fs::File::open(downloaded).map_err(|e| e.to_string())?,
                    final_file.as_file_mut(),
                )
                .map_err(|e| e.to_string())?;
                final_file.flush().map_err(|e| e.to_string())?;
                final_file.persist_noclobber(&target).map_err(|_| {
        "A file with this name already exists or couldn't be saved. Choose another folder."
    })?;
                Ok(target.to_string_lossy().into())
            },
        )
        .await
}

#[tauri::command]
async fn open_document(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    path: String,
) -> Result<()> {
    let queue = state.queue.clone();
    queue
        .execute(
            Spec::new(
                "document",
                "Open Proton document",
                drive_core::i18n::raw(path.clone()),
            )
            .interactive(),
            |_operation| async move {
                if !state.runtime.lock().unwrap().connected {
                    return Err("Connect your Proton account before opening the library.".into());
                }
                let entry = drive_core::library::entry(&state.cli, &path).await?;
                let url = drive_core::library::document_url(&entry)?;
                open_proton_window(&app, "proton-document", &url, "Proton Docs & Sheets")
            },
        )
        .await
}

#[tauri::command]
async fn setup_library(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    local_path: String,
    propagate_deletions: bool,
    include_photos: bool,
) -> Result<()> {
    let queue = state.queue.clone();
    queue.execute(Spec::new("configure", "Configure library", drive_core::i18n::raw(local_path.clone())), |_operation| async move {
    state.connect().await?;
    if !state.config.lock().unwrap().pairs.is_empty() {
        return Err("The full library overlaps existing pairings. Remove pairings on the Folders screen (your files will be preserved) and try again.".into());
    }
    let root = fs::canonicalize(local_path).map_err(|_| "Select an existing local folder.")?;
    if !root.is_dir() || root.parent().is_none() {
        return Err("Invalid local folder.".into());
    }
    state.cli.folders("/my-files").await?;
    if include_photos {
        state.cli.albums().await?;
        state.cli.photos(None).await?;
    }
    let mut pairs = Vec::new();
    let [files_name, photos_name, albums_name] = state.config.lock().unwrap().locale.library_names();
    let mut sources = vec![(files_name, "/my-files", SyncMode::Bidirectional)];
    if include_photos {
        sources.extend([
            (photos_name, "/photos", SyncMode::Bidirectional),
            (albums_name, "/albums", SyncMode::Download),
        ]);
    }
    for (name, source, mode) in sources {
        let path = drive_core::sync::safe_join(&root, name)?;
        fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        pairs.push(SyncPair {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            local_path: path.to_string_lossy().into(),
            remote_path: source.into(),
            device_uid: None,
            mode,
            interval_minutes: 5,
            enabled: true,
            last_run: None,
            propagate_deletions: source == "/my-files" && propagate_deletions,
        });
    }
    state.config.lock().unwrap().pairs = pairs;
    state.save()?;
    state.activity(&app, "success", "Library configured. Local changes sync automatically; changes in Drive are checked every 5 minutes.", None);
    Ok(())
    }).await
}

fn open_proton_window(app: &tauri::AppHandle, label: &str, url: &str, title: &str) -> Result<()> {
    let url: tauri::Url = url.parse().map_err(|_| "Invalid URL.")?;
    if let Some(window) = app.get_webview_window(label) {
        window.navigate(url).map_err(|e| e.to_string())?;
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::External(url))
        .title(title)
        .inner_size(1200., 820.)
        .center()
        .on_navigation(|url| {
            url.scheme() == "https"
                && matches!(
                    url.host_str(),
                    Some("drive.proton.me" | "account.proton.me" | "docs.proton.me")
                )
        })
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_drive(app: tauri::AppHandle) -> Result<()> {
    open_proton_window(
        &app,
        "proton-web",
        "https://drive.proton.me",
        "Proton Drive · Web",
    )
}

#[tauri::command]
fn open_recovery(state: State<'_, Arc<AppState>>, id: String) -> Result<()> {
    let root = state
        .config
        .lock()
        .unwrap()
        .pairs
        .iter()
        .find(|pair| pair.id == id)
        .map(|pair| PathBuf::from(&pair.local_path))
        .ok_or("Pairing not found.")?;
    let path = drive_core::sync::safe_join(&root, drive_core::sync::INTERNAL)?;
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    std::process::Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn open_local(state: State<'_, Arc<AppState>>, id: String) -> Result<()> {
    let path = state
        .config
        .lock()
        .unwrap()
        .pairs
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.local_path.clone())
        .ok_or("Folder not found.")?;
    std::process::Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("CapyDock")
                .build(),
        )
        .setup(|app| {
            let executable = if let Some(appimage) = app.env().appimage {
                let appimage = PathBuf::from(appimage);
                if let Err(error) =
                    desktop::align_appimage_launcher(&app.path().data_dir()?, &appimage)
                {
                    eprintln!("Could not align the CapyDock desktop entry: {error}");
                }
                appimage
            } else {
                std::env::current_exe()?
            };
            if let Err(error) = desktop::migrate_autostart(&app.path().config_dir()?, &executable) {
                eprintln!("Could not migrate the CapyDock startup entry: {error}");
            }
            let data = app.path().app_data_dir()?;
            fs::create_dir_all(data.join("bin"))?;
            let binary = data.join("bin/proton-drive");
            if !binary.exists() {
                let resources = app.path().resource_dir()?;
                let bundled = resources.join("bin/proton-drive");
                let bundled = if bundled.exists() {
                    bundled
                } else {
                    let adjacent = std::env::current_exe()?
                        .parent()
                        .ok_or("Executable has no parent directory")?
                        .join("bin/proton-drive");
                    if adjacent.exists() {
                        adjacent
                    } else {
                        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bin/proton-drive")
                    }
                };
                fs::copy(&bundled, &binary)?;
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&binary, fs::Permissions::from_mode(0o755))?;
                if let Some(parent) = bundled.parent() {
                    if parent.join("release.json").exists() {
                        fs::copy(parent.join("release.json"), data.join("bin/release.json"))?;
                    }
                }
            }
            let resource = app
                .path()
                .resource_dir()?
                .join("bin/proton-drive-computers");
            let adjacent = std::env::current_exe()?
                .parent()
                .ok_or("Executable has no parent directory")?
                .join("bin/proton-drive-computers");
            let computers_binary = if adjacent.exists() {
                adjacent
            } else if resource.exists() {
                resource
            } else {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bin/proton-drive-computers")
            };
            let config = match fs::read(data.join("settings.json")) {
                Ok(bytes) => serde_json::from_slice(&bytes)?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
                Err(e) => return Err(e.into()),
            };
            let queue_app = app.handle().clone();
            let state = Arc::new(AppState {
                catalog: drive_core::catalog::Catalog::open(data.join("library-cache.json")),
                config: Mutex::new(config),
                runtime: Mutex::new(Runtime::default()),
                queue: OperationQueue::new(move || {
                    let _ = queue_app.emit("drive-changed", ());
                }),
                data,
                cli: Cli { binary },
                computers: Cli {
                    binary: computers_binary,
                },
                tray_navigation: Mutex::new(None),
            });
            app.manage(state.clone());
            if let Err(error) = tray::create(app.handle()) {
                eprintln!("System tray unavailable: {error}");
            }
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(scheduler::run(handle, state));
            Ok(())
        })
        .on_window_event(|window, event| {
            // Auxiliary Proton windows still close normally. Hiding the main window
            // keeps the native scheduler, filesystem watcher and queue alive.
            if window.label() != "main" {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let enabled = app
                    .try_state::<Arc<AppState>>()
                    .is_some_and(|state| state.config.lock().unwrap().close_to_tray);
                // Never leave an invisible app without a tray to restore or exit it.
                if enabled && app.tray_by_id(MAIN_TRAY_ID).is_some() && window.hide().is_ok() {
                    api.prevent_close();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            take_tray_navigation,
            register_computer,
            computer_name,
            list_library,
            preview_file,
            download_file,
            open_document,
            setup_library,
            get_state,
            refresh_connection,
            login,
            logout,
            list_remote,
            create_remote_folder,
            save_pair,
            remove_pair,
            set_preferences,
            set_locale,
            sync_now,
            cancel_sync,
            cancel_operation,
            pause_operation_queue,
            clear_operation_history,
            update_cli,
            open_drive,
            open_local,
            open_recovery
        ])
        .run(tauri::generate_context!())
        .expect("Failed to start CapyDock");
}
