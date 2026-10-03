use super::AppState;
use drive_core::{
    catalog::{is_document, read_batch, FolderRequest, LibraryView, Ticket},
    cli::validate_source,
    i18n::{message, raw},
    operations::{Context, Spec, CANCELLED, PREEMPTED},
    Result,
};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};
use tauri::Emitter;

pub fn request(
    app: &tauri::AppHandle,
    state: &Arc<AppState>,
    path: String,
    force: bool,
    revision: Option<u64>,
    automatic: bool,
) -> Result<LibraryView> {
    if path != "/documents" && path != "/devices" {
        validate_source(&path)?;
    }
    if !state.runtime.lock().unwrap().connected {
        return Err("Conecte sua conta Proton antes de abrir a biblioteca.".into());
    }
    if let Some(ticket) = state.catalog.begin(&path, force) {
        let state = state.clone();
        let app = app.clone();
        let source = path.clone();
        tauri::async_runtime::spawn(async move {
            let result = loop {
                let spec = if source == "/documents" {
                    Spec::new("index", "Indexar documentos", raw(&source)).background()
                } else {
                    let mut spec =
                        Spec::new("browse", "Carregar biblioteca", raw(&source)).interactive();
                    spec.automatic = automatic;
                    spec
                };
                let result = state
                    .queue
                    .execute(spec, |operation| {
                        let (state, app, ticket, source) = (&state, &app, &ticket, &source);
                        async move {
                            if !state.catalog.valid(ticket) {
                                return Err(CANCELLED.into());
                            }
                            if source == "/documents" {
                                index(app, state, ticket, &operation).await
                            } else {
                                let entries = match source.as_str() {
                                    "/devices" => state.cli.computers().await?,
                                    "/photos" => state.cli.photos(None).await?,
                                    "/albums" => state.cli.albums().await?,
                                    p if p.starts_with("/albums/") => {
                                        state.cli.photos(Some(p)).await?
                                    }
                                    _ => state.cli.list(source).await?,
                                };
                                state.catalog.publish(ticket, entries, 1, 0, true);
                                Ok(())
                            }
                        }
                    })
                    .await;
                // Maintenance interrupts read-only processes, then the same
                // single-flight request resumes behind the exclusive barrier.
                // A user cancellation or an account change never resumes it.
                if result.as_ref().is_err_and(|e| e == PREEMPTED) && state.catalog.valid(&ticket) {
                    continue;
                }
                break result;
            };
            if let Err(error) = result {
                state.catalog.fail(&ticket, error);
            }
            let _ = app.emit("library-changed", &source);
            // Disk IO never holds the async executor or delays the IPC reply.
            let cache = state.catalog.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || cache.persist()).await;
        });
    } else if !automatic {
        state.queue.promote(&raw(&path));
    }
    Ok(state.catalog.snapshot(&path, revision))
}

pub fn preload(app: &tauri::AppHandle, state: &Arc<AppState>) {
    for path in ["/my-files", "/devices", "/albums", "/documents"] {
        let _ = request(app, state, path.into(), false, None, true);
    }
    // The full photo timeline can be very large; warm it only on demand.
}

async fn index(
    app: &tauri::AppHandle,
    state: &AppState,
    ticket: &Ticket,
    operation: &Context,
) -> Result<()> {
    let previous = state
        .catalog
        .snapshot("/documents", None)
        .entries
        .unwrap_or_default();
    let mut visible: BTreeMap<_, _> = previous.into_iter().map(|e| (e.path.clone(), e)).collect();
    let mut documents = BTreeMap::new();
    let mut pending = VecDeque::from([FolderRequest {
        path: "/my-files".into(),
        uid: None,
    }]);
    let mut seen = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut done = 0;
    let mut total = 0;
    let mut checkpoint = std::time::Instant::now();
    while !pending.is_empty() {
        if !state.catalog.valid(ticket)
            || operation.cancel.load(std::sync::atomic::Ordering::Relaxed)
        {
            return Err(CANCELLED.into());
        }
        let batch: Vec<_> = (0..4).filter_map(|_| pending.pop_front()).collect();
        for (_, entries) in read_batch(&state.computers, &batch).await? {
            done += 1;
            for entry in entries {
                total += 1;
                if total > 100_000 || entry.path.split('/').count() > 102 {
                    return Err("A pasta excede o limite de 100 níveis ou 100.000 itens.".into());
                }
                if !paths.insert(entry.path.clone()) {
                    return Err(
                        "Nomes duplicados no Drive; renomeie os arquivos antes de sincronizar."
                            .into(),
                    );
                }
                if entry.directory {
                    if !seen.insert(entry.uid.clone()) {
                        return Err("Ciclo de pastas detectado no índice.".into());
                    }
                    pending.push_back(FolderRequest {
                        path: entry.path.clone(),
                        uid: Some(entry.uid.clone()),
                    });
                } else if is_document(&entry) {
                    visible.insert(entry.path.clone(), entry.clone());
                    documents.insert(entry.path.clone(), entry);
                }
            }
        }
        let complete = pending.is_empty();
        let entries = if complete {
            documents.values().cloned().collect()
        } else {
            visible.values().cloned().collect()
        };
        state
            .catalog
            .publish(ticket, entries, done, pending.len(), complete);
        operation.progress(
            None,
            message(
                "{0} pastas verificadas · {1} documentos · {2} pastas pendentes",
                &[
                    serde_json::json!(done),
                    serde_json::json!(documents.len()),
                    serde_json::json!(pending.len()),
                ],
            ),
        );
        let _ = app.emit("library-changed", "/documents");
        // The process gate is released after each small batch, so user actions
        // and transfers get a turn instead of waiting for the whole tree.
        if !complete && checkpoint.elapsed().as_secs() >= 30 {
            let cache = state.catalog.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || cache.persist()).await;
            checkpoint = std::time::Instant::now();
        }
        tokio::task::yield_now().await;
    }
    Ok(())
}
