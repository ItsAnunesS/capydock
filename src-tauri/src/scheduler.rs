use super::{now, perform_update, run_pairs, AppState};
use drive_core::{
    operations::{Spec, CANCELLED},
    watch::LocalMonitor,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::Emitter;

pub(super) async fn run(app: tauri::AppHandle, state: Arc<AppState>) {
    let mut monitor = LocalMonitor::default();
    // Register before the first CLI call, so edits during login/startup are queued.
    reconcile(&mut monitor, &app, &state);
    let _ = state
        .queue
        .execute(
            Spec::new(
                "connection",
                "Iniciar Proton Drive",
                "Verificar CLI e conexão",
            ),
            |_| async {
                if let Ok(version) = state.cli.run(&["version"], 30).await {
                    state.runtime.lock().unwrap().cli_version = version
                        .split("cli-drive@")
                        .nth(1)
                        .and_then(|value| value.split('+').next())
                        .unwrap_or("desconhecida")
                        .to_owned();
                }
                state.connect().await
            },
        )
        .await;
    let _ = app.emit("drive-changed", ());
    super::catalog::preload(&app, &state);
    let mut preload_after = Instant::now() + Duration::from_secs(60);
    let mut update_after = Instant::now();
    let mut tick = tokio::time::interval(Duration::from_millis(500));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tick.tick().await;
        reconcile(&mut monitor, &app, &state);
        let config = state.config.lock().unwrap().clone();
        let at = Instant::now();
        if at >= preload_after {
            super::catalog::preload(&app, &state);
            preload_after = at + Duration::from_secs(60);
        }
        monitor.queue.periodic(&config, now(), at);
        let ids = monitor.queue.claim_ready(&config, at);
        if !ids.is_empty() {
            // Notify callbacks remain alive and can enqueue another pass while
            // the CLI transfers files. There is only one transfer at a time.
            if let Err(error) = run_pairs(&app, &state, Some(ids.clone()), true).await {
                if error == CANCELLED {
                    // Respect cancellation: don't immediately requeue the same timer job.
                    // Fresh filesystem events still trigger the next pass normally.
                    {
                        let mut config = state.config.lock().unwrap();
                        for pair in config.pairs.iter_mut().filter(|p| ids.contains(&p.id)) {
                            pair.last_run = Some(now());
                        }
                    }
                    let _ = state.save();
                } else {
                    monitor
                        .queue
                        .retry(&ids, Instant::now(), Duration::from_secs(30));
                }
                let _ = app.emit("drive-changed", ());
            }
            continue;
        }
        // Give local edits priority over background CLI maintenance.
        if Instant::now() >= update_after
            && config.auto_update
            && (config.paused || !monitor.queue.has_pending())
            && config
                .last_update_check
                .is_none_or(|last| now().saturating_sub(last) >= 86400)
            && perform_update(&app, &state, true)
                .await
                .is_err_and(|e| e == CANCELLED)
        {
            update_after = Instant::now() + Duration::from_secs(86400);
        }
    }
}

fn reconcile(monitor: &mut LocalMonitor, app: &tauri::AppHandle, state: &AppState) {
    let config = state.config.lock().unwrap().clone();
    for (id, message) in monitor.reconcile(&config, Instant::now()) {
        state.activity(app, "info", message, Some(id));
    }
}
