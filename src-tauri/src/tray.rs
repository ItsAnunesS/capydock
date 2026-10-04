use super::{run_pairs, show_main_window, AppState, MAIN_TRAY_ID};
use drive_core::{
    model::{Config, Locale},
    operations::{QueueView, Status, CANCELLED},
};
use std::{
    fs,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    Emitter, Listener, Manager,
};

const ICON: tauri::image::Image<'static> = tauri::include_image!("./icons/32x32.png");
const FOLDER_PREFIX: &str = "tray-folder:";

struct Labels {
    show: &'static str,
    quit: &'static str,
    sync: &'static str,
    pause: &'static str,
    resume: &'static str,
    folders: &'static str,
    settings: &'static str,
}

fn labels(locale: Locale) -> Labels {
    let (show, quit) = locale.tray_labels();
    let (sync, pause, resume, folders, settings) = match locale {
        Locale::Pt => (
            "Sincronizar agora",
            "Pausar sincronização",
            "Retomar sincronização",
            "Abrir pasta local",
            "Configurações",
        ),
        Locale::En => (
            "Sync now",
            "Pause sync",
            "Resume sync",
            "Open local folder",
            "Settings",
        ),
        Locale::Es => (
            "Sincronizar ahora",
            "Pausar sincronización",
            "Reanudar sincronización",
            "Abrir carpeta local",
            "Configuración",
        ),
    };
    Labels {
        show,
        quit,
        sync,
        pause,
        resume,
        folders,
        settings,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Folder {
    id: String,
    name: String,
    path: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Model {
    locale: Locale,
    paused: bool,
    can_sync: bool,
    can_pause: bool,
    folders: Vec<Folder>,
}

impl Model {
    fn new(config: &Config, connected: bool, queue: &QueueView, requested: bool) -> Self {
        let has_enabled = config.pairs.iter().any(|pair| pair.enabled);
        let syncing = queue.items.iter().any(|item| {
            item.kind == "sync"
                && matches!(
                    item.status,
                    Status::Queued | Status::Running | Status::Cancelling
                )
        });
        Self {
            locale: config.locale,
            paused: config.paused,
            can_sync: connected
                && has_enabled
                && !config.paused
                && !queue.paused
                && !syncing
                && !requested,
            can_pause: has_enabled || config.paused,
            // Disabled pairs still have useful local folders to open.
            folders: config
                .pairs
                .iter()
                .map(|pair| Folder {
                    id: pair.id.clone(),
                    name: pair.name.clone(),
                    path: pair.local_path.clone(),
                })
                .collect(),
        }
    }
}

struct TrayState {
    show: MenuItem<tauri::Wry>,
    sync: MenuItem<tauri::Wry>,
    pause: MenuItem<tauri::Wry>,
    folders: Submenu<tauri::Wry>,
    settings: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
    previous: Mutex<Option<Model>>,
    refresh_pending: AtomicBool,
    sync_requested: AtomicBool,
    // The file must outlive the indicator and must never share a path with
    // another application or a short-lived second CapyDock process.
    _icon_directory: tempfile::TempDir,
}

fn icon_directory(cache: &Path) -> std::io::Result<tempfile::TempDir> {
    fs::create_dir_all(cache)?;
    tempfile::Builder::new().prefix("tray-").tempdir_in(cache)
}

pub fn create(app: &tauri::AppHandle) -> tauri::Result<()> {
    let text = labels(app.state::<Arc<AppState>>().config.lock().unwrap().locale);
    let show = MenuItem::with_id(app, "show", text.show, true, None::<&str>)?;
    let sync = MenuItem::with_id(app, "tray-sync", text.sync, false, None::<&str>)?;
    let pause = MenuItem::with_id(app, "tray-pause", text.pause, false, None::<&str>)?;
    let folders = Submenu::with_id(app, "tray-folders", text.folders, false)?;
    let settings = MenuItem::with_id(app, "tray-settings", text.settings, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", text.quit, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let exit_separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &show,
            &separator,
            &sync,
            &pause,
            &folders,
            &settings,
            &exit_separator,
            &quit,
        ],
    )?;
    let directory = icon_directory(&app.path().app_cache_dir()?)?;
    tauri::tray::TrayIconBuilder::with_id(MAIN_TRAY_ID)
        .icon(ICON.clone())
        .temp_dir_path(directory.path())
        .menu(&menu)
        .tooltip("CapyDock")
        .on_menu_event(|app, event| handle_action(app, event.id.as_ref()))
        .build(app)?;
    app.manage(TrayState {
        show,
        sync,
        pause,
        folders,
        settings,
        quit,
        previous: Mutex::new(None),
        refresh_pending: AtomicBool::new(false),
        sync_requested: AtomicBool::new(false),
        _icon_directory: directory,
    });
    let handle = app.clone();
    app.listen("drive-changed", move |_| schedule_refresh(&handle));
    refresh(app)?;
    Ok(())
}

fn model(app: &tauri::AppHandle, tray: &TrayState) -> Model {
    let state = app.state::<Arc<AppState>>();
    let config = state.config.lock().unwrap().clone();
    let connected = state.runtime.lock().unwrap().connected;
    Model::new(
        &config,
        connected,
        &state.queue.snapshot(),
        tray.sync_requested.load(Ordering::Relaxed),
    )
}

fn schedule_refresh(app: &tauri::AppHandle) {
    let Some(tray) = app.try_state::<TrayState>() else {
        return;
    };
    if tray.refresh_pending.swap(true, Ordering::Relaxed) {
        return;
    }
    let handle = app.clone();
    // run_on_main_thread executes inline when called from GTK itself. Dispatch
    // from a worker so emitters holding configuration locks can always return.
    tauri::async_runtime::spawn(async move {
        let scheduled = handle.clone();
        if handle
            .run_on_main_thread(move || {
                if let Some(tray) = scheduled.try_state::<TrayState>() {
                    tray.refresh_pending.store(false, Ordering::Relaxed);
                    if let Err(error) = refresh(&scheduled) {
                        eprintln!("Could not refresh the CapyDock tray: {error}");
                    }
                }
            })
            .is_err()
        {
            if let Some(tray) = handle.try_state::<TrayState>() {
                tray.refresh_pending.store(false, Ordering::Relaxed);
            }
        }
    });
}

fn refresh(app: &tauri::AppHandle) -> tauri::Result<()> {
    let tray = app.state::<TrayState>();
    let next = model(app, &tray);
    let mut previous = tray.previous.lock().unwrap();
    if previous.as_ref() == Some(&next) {
        return Ok(());
    }
    let text = labels(next.locale);
    tray.show.set_text(text.show)?;
    tray.quit.set_text(text.quit)?;
    tray.sync.set_text(text.sync)?;
    tray.sync.set_enabled(next.can_sync)?;
    tray.pause
        .set_text(if next.paused { text.resume } else { text.pause })?;
    tray.pause.set_enabled(next.can_pause)?;
    tray.settings.set_text(text.settings)?;
    tray.folders.set_text(text.folders)?;
    tray.folders.set_enabled(!next.folders.is_empty())?;
    if previous
        .as_ref()
        .is_none_or(|old| old.folders != next.folders)
    {
        for item in tray.folders.items()? {
            tray.folders.remove(&item)?;
        }
        for folder in &next.folders {
            let title = if next
                .folders
                .iter()
                .filter(|other| other.name == folder.name)
                .count()
                > 1
            {
                format!("{} — {}", folder.name, folder.path)
            } else {
                folder.name.clone()
            };
            // GTK treats '&' as a mnemonic. Folder names are user data, not labels.
            let item = MenuItem::with_id(
                app,
                format!("{FOLDER_PREFIX}{}", folder.id),
                title.replace('&', "&&"),
                true,
                None::<&str>,
            )?;
            tray.folders.append(&item)?;
        }
    }
    *previous = Some(next);
    Ok(())
}

fn report_error(app: &tauri::AppHandle, error: String) {
    if error != CANCELLED {
        app.state::<Arc<AppState>>()
            .activity(app, "error", error.clone(), None);
        let _ = app.emit_to("main", "tray-error", error);
    }
}

fn handle_action(app: &tauri::AppHandle, id: &str) {
    match id {
        "show" => show_main_window(app),
        "quit" => app.exit(0),
        "tray-settings" => {
            *app.state::<Arc<AppState>>().tray_navigation.lock().unwrap() = Some("settings");
            show_main_window(app);
            let _ = app.emit_to("main", "tray-navigation", ());
        }
        "tray-pause" => {
            let state = app.state::<Arc<AppState>>();
            let paused = !state.config.lock().unwrap().paused;
            if let Err(error) = state.set_sync_paused(paused) {
                report_error(app, error);
            }
            let _ = app.emit("drive-changed", ());
        }
        "tray-sync" => {
            let tray = app.state::<TrayState>();
            if !model(app, &tray).can_sync || tray.sync_requested.swap(true, Ordering::Relaxed) {
                return;
            }
            schedule_refresh(app);
            let handle = app.clone();
            let state = app.state::<Arc<AppState>>().inner().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = run_pairs(&handle, &state, None, false).await {
                    report_error(&handle, error);
                }
                handle
                    .state::<TrayState>()
                    .sync_requested
                    .store(false, Ordering::Relaxed);
                schedule_refresh(&handle);
            });
        }
        _ => {
            if let Some(id) = id.strip_prefix(FOLDER_PREFIX) {
                if let Err(error) = super::open_local(app.state::<Arc<AppState>>(), id.to_owned()) {
                    report_error(app, error);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use drive_core::{
        model::{SyncMode, SyncPair},
        operations::{Lane, Operation},
    };

    fn config() -> Config {
        Config {
            pairs: vec![SyncPair {
                id: "docs".into(),
                name: "Documents".into(),
                local_path: "/tmp/docs".into(),
                remote_path: "/my-files/docs".into(),
                device_uid: None,
                mode: SyncMode::Bidirectional,
                interval_minutes: 5,
                enabled: true,
                last_run: None,
                propagate_deletions: false,
            }],
            ..Config::default()
        }
    }

    #[test]
    fn sync_requires_an_enabled_folder_connection_and_unpaused_queue() {
        let mut config = config();
        let mut queue = QueueView {
            paused: false,
            items: vec![],
        };
        assert!(Model::new(&config, true, &queue, false).can_sync);
        assert!(!Model::new(&config, false, &queue, false).can_sync);
        assert!(!Model::new(&config, true, &queue, true).can_sync);
        queue.paused = true;
        assert!(!Model::new(&config, true, &queue, false).can_sync);
        queue.paused = false;
        config.paused = true;
        assert!(!Model::new(&config, true, &queue, false).can_sync);
        assert!(Model::new(&config, true, &queue, false).can_pause);
        config.paused = false;
        config.pairs[0].enabled = false;
        let model = Model::new(&config, true, &queue, false);
        assert!(!model.can_sync);
        assert!(!model.can_pause);
        assert_eq!(model.folders[0].id, "docs");
    }

    #[test]
    fn pending_syncs_cannot_be_duplicated_but_completed_ones_can_be_retried() {
        let config = config();
        for status in [
            Status::Queued,
            Status::Running,
            Status::Cancelling,
            Status::Completed,
            Status::Failed,
            Status::Cancelled,
        ] {
            let queue = QueueView {
                paused: false,
                items: vec![Operation {
                    id: "sync".into(),
                    kind: "sync".into(),
                    lane: Lane::Transfer,
                    title: String::new(),
                    detail: String::new(),
                    pair_id: None,
                    automatic: false,
                    status,
                    created_at: 0,
                    started_at: None,
                    finished_at: None,
                    error: None,
                    can_cancel_running: true,
                }],
            };
            assert_eq!(
                Model::new(&config, true, &queue, false).can_sync,
                matches!(
                    status,
                    Status::Completed | Status::Failed | Status::Cancelled
                )
            );
        }
    }

    #[test]
    fn closing_another_instance_cannot_remove_the_active_tray_icon() {
        let cache = tempfile::tempdir().unwrap();
        let running = icon_directory(cache.path()).unwrap();
        let other = icon_directory(cache.path()).unwrap();
        assert_ne!(running.path(), other.path());
        let path = running.path().join("tray-icon-main-tray-0.png");
        fs::write(&path, include_bytes!("../icons/32x32.png")).unwrap();
        drop(other);
        assert_eq!(
            fs::read(path).unwrap(),
            include_bytes!("../icons/32x32.png")
        );
        assert_eq!((ICON.width(), ICON.height()), (32, 32));
        assert_eq!(ICON.rgba().len(), 32 * 32 * 4);
    }

    #[test]
    fn every_packaged_icon_is_rgba8_for_tauri_and_appindicator() {
        for png in [
            include_bytes!("../icons/32x32.png").as_slice(),
            include_bytes!("../icons/128x128.png").as_slice(),
            include_bytes!("../icons/icon.png").as_slice(),
        ] {
            assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
            assert_eq!(
                png[24], 8,
                "Tauri embeds 16-bit PNG samples without converting them to RGBA8"
            );
            assert_eq!(png[25], 6, "Icons must include the alpha channel");
        }
    }

    #[test]
    fn all_menu_actions_follow_the_selected_language() {
        for (locale, expected) in [
            (
                Locale::Pt,
                [
                    "Sincronizar agora",
                    "Pausar sincronização",
                    "Retomar sincronização",
                    "Abrir pasta local",
                    "Configurações",
                ],
            ),
            (
                Locale::En,
                [
                    "Sync now",
                    "Pause sync",
                    "Resume sync",
                    "Open local folder",
                    "Settings",
                ],
            ),
            (
                Locale::Es,
                [
                    "Sincronizar ahora",
                    "Pausar sincronización",
                    "Reanudar sincronización",
                    "Abrir carpeta local",
                    "Configuración",
                ],
            ),
        ] {
            let text = labels(locale);
            assert_eq!(
                [
                    text.sync,
                    text.pause,
                    text.resume,
                    text.folders,
                    text.settings
                ],
                expected
            );
        }
    }

    #[test]
    fn pause_persists_without_changing_other_preferences_and_rolls_back_on_write_failure() {
        let directory = tempfile::tempdir().unwrap();
        let config = Config {
            auto_update: false,
            close_to_tray: true,
            locale: Locale::Es,
            ..config()
        };
        let mut state = AppState {
            config: Mutex::new(config.clone()),
            catalog: drive_core::catalog::Catalog::open(directory.path().join("catalog.json")),
            runtime: Mutex::new(super::super::Runtime::default()),
            queue: drive_core::operations::OperationQueue::new(|| {}),
            data: directory.path().to_path_buf(),
            cli: drive_core::cli::Cli {
                binary: "unused".into(),
            },
            computers: drive_core::cli::Cli {
                binary: "unused".into(),
            },
            tray_navigation: Mutex::new(None),
        };
        state.set_sync_paused(true).unwrap();
        let saved: Config =
            serde_json::from_slice(&fs::read(directory.path().join("settings.json")).unwrap())
                .unwrap();
        assert!(saved.paused);
        assert!(saved.close_to_tray);
        assert!(!saved.auto_update);
        assert_eq!(saved.locale, Locale::Es);
        state.set_sync_paused(false).unwrap();
        assert_eq!(
            serde_json::to_value(&*state.config.lock().unwrap()).unwrap(),
            serde_json::to_value(&config).unwrap()
        );
        state.data = directory.path().join("not-a-directory");
        fs::write(&state.data, b"existing file").unwrap();
        assert!(state.set_sync_paused(true).is_err());
        assert!(!state.config.lock().unwrap().paused);
    }
}
