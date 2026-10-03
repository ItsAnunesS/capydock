//! Local display-cache benchmark. No network, credentials, or user files.
use drive_core::{catalog::Catalog, model::RemoteEntry};
use std::{hint::black_box, time::Instant};
fn main() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("catalog.json");
    let cache = Catalog::open(file.clone());
    cache.activate("fixture-account");
    let ticket = cache.begin("/documents", false).unwrap();
    let documents: Vec<_> = (0..10_000)
        .map(|i| RemoteEntry {
            name: format!("Document {i}.pdf"),
            path: format!("/my-files/Folder/Document {i}.pdf"),
            uid: format!("node-{i}"),
            media_type: "application/pdf".into(),
            ..RemoteEntry::default()
        })
        .collect();
    cache.publish(&ticket, documents, 1000, 0, true);
    cache.persist().unwrap();
    let start = Instant::now();
    let loaded = Catalog::open(file);
    loaded.activate("fixture-account");
    let disk_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let view = loaded.snapshot("/documents", None);
    let snapshot_ms = start.elapsed().as_secs_f64() * 1000.0;
    let bytes = serde_json::to_vec(&view).unwrap().len();
    let start = Instant::now();
    for _ in 0..1000 {
        black_box(loaded.snapshot("/documents", Some(view.revision)));
    }
    let unchanged_us = start.elapsed().as_secs_f64() * 1000.0;
    let tiny = serde_json::to_vec(&loaded.snapshot("/documents", Some(view.revision)))
        .unwrap()
        .len();
    println!(
        "{}",
        serde_json::json!({"scope":"synthetic display cache, no network or authenticated account", "documents":10000,"disk_load_ms":disk_ms,"first_snapshot_ms":snapshot_ms,"unchanged_snapshot_mean_us":unchanged_us,"full_ipc_bytes":bytes,"unchanged_ipc_bytes":tiny})
    );
}
