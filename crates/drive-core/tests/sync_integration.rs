use drive_core::{
    cli::Cli,
    model::{Snapshot, SyncMode, SyncPair},
    sync::synchronize,
};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, sync::atomic::AtomicBool};

struct Fixture {
    _root: tempfile::TempDir,
    local: PathBuf,
    remote: PathBuf,
    snapshot: PathBuf,
    cli: Cli,
    pair: SyncPair,
    cancel: AtomicBool,
}
impl Fixture {
    fn new(mode: SyncMode) -> Self {
        let root = tempfile::tempdir().unwrap();
        let local = root.path().join("local");
        let remote = root.path().join("remote");
        fs::create_dir(&local).unwrap();
        fs::create_dir(&remote).unwrap();
        let binary = root.path().join("fake_cli.py");
        fs::write(&binary, include_str!("fixtures/fake_cli.py")).unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o755)).unwrap();
        let pair = SyncPair {
            id: "fixture".into(),
            name: "Test".into(),
            local_path: local.to_str().unwrap().into(),
            remote_path: "/my-files".into(),
            device_uid: None,
            mode,
            interval_minutes: 5,
            enabled: true,
            last_run: None,
            propagate_deletions: false,
        };
        let snapshot = root.path().join("snapshot.json");
        Self {
            _root: root,
            local,
            remote,
            snapshot,
            cli: Cli { binary },
            pair,
            cancel: AtomicBool::new(false),
        }
    }
    async fn run(&self) -> drive_core::Result<drive_core::model::SyncReport> {
        synchronize(
            &self.cli,
            &self.pair,
            &self.snapshot,
            &self.cancel,
            |_, _| {},
        )
        .await
    }
    fn snapshot(&self) -> Snapshot {
        serde_json::from_slice(&fs::read(&self.snapshot).unwrap()).unwrap()
    }
}

#[tokio::test]
async fn incremental_roundtrip_and_conflicts() {
    let f = Fixture::new(SyncMode::Bidirectional);
    fs::create_dir(f.local.join("nested")).unwrap();
    fs::write(f.local.join("nested/a.txt"), "original").unwrap();
    let first = f.run().await.unwrap();
    assert_eq!(first.uploaded, 2);
    assert_eq!(
        fs::read_to_string(f.remote.join("nested/a.txt")).unwrap(),
        "original"
    );
    let second = f.run().await.unwrap();
    assert_eq!(second.uploaded, 0);
    assert_eq!(second.downloaded, 0);
    fs::write(f.local.join("nested/a.txt"), "local edit").unwrap();
    assert_eq!(f.run().await.unwrap().uploaded, 1);
    fs::write(f.remote.join("nested/a.txt"), "remote edit").unwrap();
    assert_eq!(f.run().await.unwrap().downloaded, 1);
    assert_eq!(
        fs::read_to_string(f.local.join("nested/a.txt")).unwrap(),
        "remote edit"
    );
    let backup = walkdir::WalkDir::new(f.local.join(".proton-drive-desktop/backups"))
        .into_iter()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name() == "a.txt")
        .unwrap();
    assert_eq!(fs::read_to_string(backup.path()).unwrap(), "local edit");
    fs::write(f.local.join("nested/a.txt"), "local conflict").unwrap();
    fs::write(f.remote.join("nested/a.txt"), "remote conflict").unwrap();
    let conflict = f.run().await.unwrap();
    assert_eq!(conflict.conflicts, vec!["nested/a.txt"]);
    assert_eq!(
        fs::read_to_string(f.local.join("nested/a.txt")).unwrap(),
        "local conflict"
    );
    assert_eq!(
        fs::read_to_string(f.remote.join("nested/a.txt")).unwrap(),
        "remote conflict"
    );
}

#[tokio::test]
async fn existing_equal_files_establish_baseline_without_transfer() {
    let f = Fixture::new(SyncMode::Bidirectional);
    fs::write(f.local.join("same.txt"), "same").unwrap();
    fs::write(f.remote.join("same.txt"), "same").unwrap();
    let report = f.run().await.unwrap();
    assert_eq!(report.unchanged, 1);
    assert!(report.conflicts.is_empty());
    assert_eq!(f.snapshot().len(), 1);
}

#[tokio::test]
async fn aligning_conflicting_copies_resolves_without_resetting_other_files() {
    let f = Fixture::new(SyncMode::Bidirectional);
    fs::write(f.local.join("a.txt"), "initial").unwrap();
    f.run().await.unwrap();
    fs::write(f.local.join("a.txt"), "chosen").unwrap();
    fs::write(f.remote.join("a.txt"), "other").unwrap();
    assert_eq!(f.run().await.unwrap().conflicts.len(), 1);
    fs::write(f.remote.join("a.txt"), "chosen").unwrap();
    let report = f.run().await.unwrap();
    assert!(report.conflicts.is_empty());
    assert_eq!(report.unchanged, 1);
    fs::write(f.local.join("a.txt"), "next edit").unwrap();
    assert_eq!(f.run().await.unwrap().uploaded, 1);
}

#[tokio::test]
async fn existing_different_files_are_never_overwritten_on_first_run() {
    let f = Fixture::new(SyncMode::Bidirectional);
    fs::write(f.local.join("a.txt"), "local").unwrap();
    fs::write(f.remote.join("a.txt"), "remote").unwrap();
    assert_eq!(f.run().await.unwrap().conflicts, vec!["a.txt"]);
    assert!(!f.snapshot.exists());
    assert_eq!(fs::read_to_string(f.local.join("a.txt")).unwrap(), "local");
}

#[tokio::test]
async fn remote_deletion_preserves_local_and_reports_conflict() {
    let f = Fixture::new(SyncMode::Bidirectional);
    fs::write(f.local.join("a.txt"), "keep").unwrap();
    f.run().await.unwrap();
    fs::remove_file(f.remote.join("a.txt")).unwrap();
    assert_eq!(f.run().await.unwrap().conflicts, vec!["a.txt"]);
    assert!(f.local.join("a.txt").exists());
    assert!(!f.remote.join("a.txt").exists());
}

#[tokio::test]
async fn local_deletion_does_not_delete_remote_or_silently_restore() {
    let f = Fixture::new(SyncMode::Bidirectional);
    fs::write(f.remote.join("a.txt"), "keep").unwrap();
    f.run().await.unwrap();
    fs::remove_file(f.local.join("a.txt")).unwrap();
    assert_eq!(f.run().await.unwrap().conflicts, vec!["a.txt"]);
    assert!(f.remote.join("a.txt").exists());
    assert!(!f.local.join("a.txt").exists());
}

#[tokio::test]
async fn failed_transfer_does_not_commit_snapshot_and_retry_recovers() {
    let f = Fixture::new(SyncMode::Upload);
    fs::write(f.local.join("a.txt"), "original").unwrap();
    f.run().await.unwrap();
    let before = fs::read(&f.snapshot).unwrap();
    fs::write(f.local.join("a.txt"), "change").unwrap();
    let fail = f.cli.binary.parent().unwrap().join("fail-transfer");
    fs::write(&fail, "").unwrap();
    assert!(f.run().await.is_err());
    assert_eq!(fs::read(&f.snapshot).unwrap(), before);
    assert_eq!(
        fs::read_to_string(f.remote.join("a.txt")).unwrap(),
        "original"
    );
    fs::remove_file(fail).unwrap();
    assert_eq!(f.run().await.unwrap().uploaded, 1);
}

#[tokio::test]
async fn one_way_modes_never_transfer_opposite_direction() {
    for mode in [SyncMode::Upload, SyncMode::Download] {
        let f = Fixture::new(mode);
        fs::write(f.local.join("local.txt"), "local").unwrap();
        fs::write(f.remote.join("remote.txt"), "remote").unwrap();
        let report = f.run().await.unwrap();
        if mode == SyncMode::Upload {
            assert_eq!(report.downloaded, 0);
            assert!(!f.local.join("remote.txt").exists());
        } else {
            assert_eq!(report.uploaded, 0);
            assert!(!f.remote.join("local.txt").exists());
        }
    }
}

#[tokio::test]
async fn remote_symlink_collision_never_writes_outside_selected_folder() {
    let f = Fixture::new(SyncMode::Download);
    let outside = tempfile::tempdir().unwrap();
    fs::create_dir(f.remote.join("escape")).unwrap();
    fs::write(f.remote.join("escape/secret.txt"), "inbound").unwrap();
    std::os::unix::fs::symlink(outside.path(), f.local.join("escape")).unwrap();
    assert!(f.run().await.is_err());
    assert!(!outside.path().join("secret.txt").exists());
}

#[tokio::test]
async fn cancellation_never_starts_a_transfer() {
    let f = Fixture::new(SyncMode::Upload);
    fs::write(f.local.join("a.txt"), "local").unwrap();
    f.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(f.run().await.is_err());
    assert!(!f.remote.join("a.txt").exists());
}

#[tokio::test]
async fn folder_picker_filters_files_at_the_cli() {
    let f = Fixture::new(SyncMode::Bidirectional);
    fs::create_dir(f.remote.join("Documentos")).unwrap();
    fs::write(f.remote.join("file.txt"), "content").unwrap();
    let folders = f.cli.folders("/my-files").await.unwrap();
    assert_eq!(folders.len(), 1);
    assert_eq!(folders[0].name, "Documentos");
    assert!(folders[0].directory);
}

#[tokio::test]
async fn explicit_deletion_uses_remote_trash_and_local_recovery() {
    let mut f = Fixture::new(SyncMode::Bidirectional);
    f.pair.propagate_deletions = true;
    fs::write(f.local.join("a.txt"), "local original").unwrap();
    fs::write(f.remote.join("b.txt"), "remote original").unwrap();
    f.run().await.unwrap();
    fs::remove_file(f.local.join("a.txt")).unwrap();
    fs::remove_file(f.remote.join("b.txt")).unwrap();
    let report = f.run().await.unwrap();
    assert_eq!(report.deleted, 2);
    assert_eq!(
        fs::read_to_string(f._root.path().join("trash/a.txt")).unwrap(),
        "local original"
    );
    let recovery = walkdir::WalkDir::new(f.local.join(".proton-drive-desktop/trash"))
        .into_iter()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name() == "b.txt")
        .unwrap();
    assert_eq!(
        fs::read_to_string(recovery.path()).unwrap(),
        "remote original"
    );
    assert!(!f.local.join("b.txt").exists());
    assert!(!f.remote.join("a.txt").exists());
    assert!(f.snapshot().is_empty());
}

#[tokio::test]
async fn directory_deletion_preserves_changed_descendants() {
    let mut f = Fixture::new(SyncMode::Bidirectional);
    f.pair.propagate_deletions = true;
    fs::create_dir(f.local.join("folder")).unwrap();
    fs::write(f.local.join("folder/file.txt"), "original").unwrap();
    f.run().await.unwrap();
    fs::remove_dir_all(f.local.join("folder")).unwrap();
    fs::write(f.remote.join("folder/file.txt"), "edited remotely").unwrap();
    let report = f.run().await.unwrap();
    assert_eq!(report.deleted, 0);
    assert_eq!(report.conflicts, ["folder"]);
    assert_eq!(
        fs::read_to_string(f.remote.join("folder/file.txt")).unwrap(),
        "edited remotely"
    );
}

#[tokio::test]
async fn unchanged_directory_deletion_moves_entire_tree_to_recovery() {
    let mut f = Fixture::new(SyncMode::Bidirectional);
    f.pair.propagate_deletions = true;
    fs::create_dir_all(f.local.join("folder/nested")).unwrap();
    fs::write(f.local.join("folder/nested/file.txt"), "original").unwrap();
    f.run().await.unwrap();
    fs::remove_dir_all(f.remote.join("folder")).unwrap();
    let report = f.run().await.unwrap();
    assert_eq!(report.deleted, 1);
    assert!(f.snapshot().is_empty());
    assert!(!f.local.join("folder").exists());
    assert!(
        walkdir::WalkDir::new(f.local.join(".proton-drive-desktop/trash"))
            .into_iter()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name() == "file.txt")
    );
}

#[tokio::test]
async fn rename_transfers_new_copy_before_trashing_old_and_failure_keeps_old() {
    let mut f = Fixture::new(SyncMode::Bidirectional);
    f.pair.propagate_deletions = true;
    fs::write(f.local.join("a.txt"), "original").unwrap();
    f.run().await.unwrap();
    fs::rename(f.local.join("a.txt"), f.local.join("z.txt")).unwrap();
    fs::write(f._root.path().join("fail-transfer"), "").unwrap();
    assert!(f.run().await.is_err());
    assert!(f.remote.join("a.txt").exists());
    fs::remove_file(f._root.path().join("fail-transfer")).unwrap();
    let report = f.run().await.unwrap();
    assert_eq!(report.uploaded, 1);
    assert_eq!(report.deleted, 1);
    assert_eq!(
        fs::read_to_string(f.remote.join("z.txt")).unwrap(),
        "original"
    );
}

#[tokio::test]
async fn unsuccessful_trash_and_failed_listing_do_not_advance_deletion_history() {
    let mut f = Fixture::new(SyncMode::Bidirectional);
    f.pair.propagate_deletions = true;
    fs::write(f.local.join("a.txt"), "original").unwrap();
    f.run().await.unwrap();
    fs::remove_file(f.local.join("a.txt")).unwrap();
    fs::write(f._root.path().join("fail-list"), "").unwrap();
    assert!(f.run().await.is_err());
    assert!(f.remote.join("a.txt").exists());
    fs::remove_file(f._root.path().join("fail-list")).unwrap();
    fs::write(f._root.path().join("fail-trash"), "").unwrap();
    assert!(f.run().await.is_err());
    assert!(f.snapshot().contains_key("a.txt"));
    assert!(f.remote.join("a.txt").exists());
}

#[tokio::test]
async fn native_documents_do_not_block_other_files() {
    let f = Fixture::new(SyncMode::Bidirectional);
    fs::write(f.remote.join("Online.protondoc"), "native").unwrap();
    fs::write(f.remote.join("file.txt"), "download").unwrap();
    let report = f.run().await.unwrap();
    assert_eq!(report.online_only, 1);
    assert_eq!(report.downloaded, 1);
    assert!(!f.local.join("Online.protondoc").exists());
    assert_eq!(
        fs::read_to_string(f.local.join("file.txt")).unwrap(),
        "download"
    );
}

impl Fixture {
    fn photo(&self, uid: &str, name: &str, contents: &str) {
        let folder = self._root.path().join("photos").join(uid);
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join(name), contents).unwrap();
    }
    fn albums(&self, value: serde_json::Value) {
        fs::write(
            self._root.path().join("albums.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }
}

#[tokio::test]
async fn photos_with_duplicate_names_stay_distinct_and_incremental() {
    let mut f = Fixture::new(SyncMode::Bidirectional);
    f.pair.remote_path = "/photos".into();
    f.photo("one", "IMG.jpg", "one");
    f.photo("two", "IMG.jpg", "two");
    assert_eq!(f.run().await.unwrap().downloaded, 2);
    let files: Vec<_> = fs::read_dir(&f.local)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .collect();
    assert_eq!(files.len(), 2);
    assert_ne!(files[0].file_name(), files[1].file_name());
    let next = f.run().await.unwrap();
    assert_eq!(next.downloaded, 0);
    assert_eq!(next.uploaded, 0);
    assert_eq!(next.unchanged, 2);
    fs::write(files[0].path(), "local edit").unwrap();
    assert_eq!(f.run().await.unwrap().conflicts.len(), 1);
}

#[tokio::test]
async fn new_albums_and_members_are_discovered_without_reconfiguration() {
    let mut f = Fixture::new(SyncMode::Download);
    f.pair.remote_path = "/albums".into();
    f.photo("one", "IMG.jpg", "one");
    f.photo("two", "IMG.jpg", "two");
    f.albums(serde_json::json!({"album-one": {"name":"Viagem", "photos":["one"]}}));
    assert_eq!(f.run().await.unwrap().downloaded, 1);
    f.albums(serde_json::json!({"album-one": {"name":"Viagem", "photos":["one", "two"]}, "album-two": {"name":"Viagem", "photos":["two"]}}));
    let report = f.run().await.unwrap();
    assert_eq!(report.downloaded, 2);
    assert_eq!(report.unchanged, 1);
    assert_eq!(f.run().await.unwrap().downloaded, 0);
}

#[tokio::test]
async fn album_new_local_photo_uploads_and_membership_is_verified_without_duplicate_download() {
    let mut f = Fixture::new(SyncMode::Bidirectional);
    f.pair.remote_path = "/albums/album-one".into();
    fs::create_dir(f._root.path().join("photos")).unwrap();
    f.albums(serde_json::json!({"album-one": {"name":"Viagem", "photos":[]}}));
    fs::write(f.local.join("new.jpg"), "local photo").unwrap();
    assert_eq!(f.run().await.unwrap().uploaded, 1);
    let photos = f.cli.photos(Some("/albums/album-one")).await.unwrap();
    assert_eq!(photos.len(), 1);
    let next = f.run().await.unwrap();
    assert_eq!(next.uploaded, 0);
    assert_eq!(next.downloaded, 0);
    assert_eq!(next.unchanged, 1);
    fs::remove_file(f.local.join("new.jpg")).unwrap();
    let next = f.run().await.unwrap();
    assert_eq!(next.conflicts.len(), 1);
    assert_eq!(
        f.cli.photos(Some("/albums/album-one")).await.unwrap().len(),
        1
    );
}

#[tokio::test]
async fn ignored_children_and_root_symlinks_block_automatic_removal() {
    let mut f = Fixture::new(SyncMode::Bidirectional);
    f.pair.propagate_deletions = true;
    fs::create_dir_all(f.local.join("folder")).unwrap();
    fs::write(f.local.join("folder/a.txt"), "original").unwrap();
    f.run().await.unwrap();
    fs::remove_dir_all(f.local.join("folder")).unwrap();
    fs::create_dir(f.remote.join("folder/.git")).unwrap();
    assert!(f.run().await.unwrap_err().contains("ignorados"));
    assert!(f.remote.join("folder/a.txt").exists());
    let alternate = f._root.path().join("alternate");
    fs::rename(&f.local, &alternate).unwrap();
    std::os::unix::fs::symlink(&alternate, &f.local).unwrap();
    assert!(f.run().await.unwrap_err().contains("simbólico"));
}

async fn wait_for_local_change(
    monitor: &drive_core::watch::LocalMonitor,
    config: &drive_core::model::Config,
) {
    use std::time::{Duration, Instant};
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if monitor
            .queue
            .claim_ready(config, Instant::now())
            .contains(&config.pairs[0].id)
        {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Native filesystem change did not trigger automatic sync"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn native_watcher_syncs_folder_creation_edits_atomic_saves_renames_and_deletions() {
    use drive_core::{model::Config, watch::LocalMonitor};
    use std::time::Instant;
    let mut f = Fixture::new(SyncMode::Bidirectional);
    f.pair.propagate_deletions = true;
    f.pair.last_run = Some(drive_core::now()); // Periodic sync is five minutes away.
    let config = Config {
        pairs: vec![f.pair.clone()],
        ..Config::default()
    };
    let mut monitor = LocalMonitor::default();
    assert!(monitor.reconcile(&config, Instant::now()).is_empty());
    monitor.queue.claim_ready(&config, Instant::now()); // Consume startup reconciliation.

    fs::create_dir(f.local.join("new-folder")).unwrap();
    wait_for_local_change(&monitor, &config).await;
    assert_eq!(f.run().await.unwrap().uploaded, 1);
    assert!(f.remote.join("new-folder").is_dir());

    fs::write(f.local.join("new-folder/file.txt"), "new").unwrap();
    wait_for_local_change(&monitor, &config).await;
    assert_eq!(f.run().await.unwrap().uploaded, 1);

    fs::write(f.local.join("new-folder/file.txt"), "edited").unwrap();
    wait_for_local_change(&monitor, &config).await;
    assert_eq!(f.run().await.unwrap().uploaded, 1);
    assert_eq!(
        fs::read_to_string(f.remote.join("new-folder/file.txt")).unwrap(),
        "edited"
    );

    // Editors frequently save a temporary file and atomically replace the original.
    fs::write(f._root.path().join("editor-save"), "atomic edit").unwrap();
    fs::rename(
        f._root.path().join("editor-save"),
        f.local.join("new-folder/file.txt"),
    )
    .unwrap();
    wait_for_local_change(&monitor, &config).await;
    assert_eq!(f.run().await.unwrap().uploaded, 1);
    assert_eq!(
        fs::read_to_string(f.remote.join("new-folder/file.txt")).unwrap(),
        "atomic edit"
    );

    fs::rename(
        f.local.join("new-folder/file.txt"),
        f.local.join("new-folder/renamed.txt"),
    )
    .unwrap();
    wait_for_local_change(&monitor, &config).await;
    let renamed = f.run().await.unwrap();
    assert_eq!(renamed.uploaded, 1);
    assert_eq!(renamed.deleted, 1);

    fs::remove_file(f.local.join("new-folder/renamed.txt")).unwrap();
    wait_for_local_change(&monitor, &config).await;
    assert_eq!(f.run().await.unwrap().deleted, 1);
    assert!(!f.remote.join("new-folder/renamed.txt").exists());
}

#[tokio::test]
async fn watcher_download_feedback_settles_and_internal_writes_do_not_loop() {
    use drive_core::{model::Config, watch::LocalMonitor};
    use std::time::{Duration, Instant};
    let f = Fixture::new(SyncMode::Bidirectional);
    let config = Config {
        pairs: vec![f.pair.clone()],
        ..Config::default()
    };
    let mut monitor = LocalMonitor::default();
    monitor.reconcile(&config, Instant::now());
    monitor.queue.claim_ready(&config, Instant::now());
    fs::write(f.remote.join("remote.txt"), "download").unwrap();
    f.run().await.unwrap();
    // The app's download creates a real local event; one follow-up reconciles it.
    wait_for_local_change(&monitor, &config).await;
    let follow_up = f.run().await.unwrap();
    assert_eq!(follow_up.downloaded, 0);
    assert_eq!(follow_up.uploaded, 0);
    fs::create_dir_all(f.local.join(".proton-drive-desktop/backups")).unwrap();
    fs::write(
        f.local.join(".proton-drive-desktop/backups/ignore.txt"),
        "internal",
    )
    .unwrap();
    let _ = fs::read(f.local.join("remote.txt")).unwrap();
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert!(!monitor.queue.has_pending());
}

#[tokio::test]
async fn watcher_reconciles_new_pairs_pause_resume_and_replaced_roots() {
    use drive_core::{model::Config, watch::LocalMonitor};
    use std::time::Instant;
    let f = Fixture::new(SyncMode::Bidirectional);
    let mut config = Config {
        pairs: vec![f.pair.clone()],
        ..Config::default()
    };
    let mut monitor = LocalMonitor::default();
    monitor.reconcile(&config, Instant::now());
    monitor.queue.claim_ready(&config, Instant::now());
    config.paused = true;
    monitor.reconcile(&config, Instant::now());
    fs::write(f.local.join("paused.txt"), "waiting").unwrap();
    assert!(monitor
        .queue
        .claim_ready(&config, Instant::now())
        .is_empty());
    config.paused = false;
    monitor.reconcile(&config, Instant::now());
    wait_for_local_change(&monitor, &config).await;

    fs::rename(&f.local, f._root.path().join("old-root")).unwrap();
    monitor.reconcile(&config, Instant::now());
    fs::create_dir(&f.local).unwrap();
    assert!(monitor.reconcile(&config, Instant::now()).is_empty());
    monitor.queue.claim_ready(&config, Instant::now());
    fs::write(f.local.join("reconnected.txt"), "new root").unwrap();
    wait_for_local_change(&monitor, &config).await;

    config.pairs[0].enabled = false;
    monitor.reconcile(&config, Instant::now());
    fs::write(f.local.join("disabled.txt"), "disabled").unwrap();
    assert!(!monitor.queue.has_pending());
    config.pairs[0].enabled = true;
    monitor.reconcile(&config, Instant::now());
    assert!(!monitor
        .queue
        .claim_ready(&config, Instant::now())
        .is_empty());
    config.pairs.clear();
    monitor.reconcile(&config, Instant::now());
    assert!(!monitor.queue.has_pending());
}

#[tokio::test]
async fn computers_sync_both_directions_and_bind_to_device_identity() {
    let mut f = Fixture::new(SyncMode::Bidirectional);
    let devices = f._root.path().join("devices.json");
    let device = serde_json::json!({"uid":"pc-1","rootFolderUid":"root-1","type":"Linux","name":{"ok":true,"value":"Meu PC"}});
    fs::write(&devices, serde_json::to_vec(&vec![device.clone()]).unwrap()).unwrap();
    let folder = f.remote.join("_devices/Meu PC/Documentos");
    fs::create_dir_all(&folder).unwrap();
    f.pair.remote_path = "/devices/Meu PC/Documentos".into();
    f.pair.device_uid = Some("pc-1".into());
    fs::write(f.local.join("local.txt"), "enviado").unwrap();
    fs::write(folder.join("remote.txt"), "recebido").unwrap();
    let report = f.run().await.unwrap();
    assert_eq!(report.uploaded, 1);
    assert_eq!(report.downloaded, 1);
    assert_eq!(
        fs::read_to_string(folder.join("local.txt")).unwrap(),
        "enviado"
    );
    assert_eq!(
        fs::read_to_string(f.local.join("remote.txt")).unwrap(),
        "recebido"
    );
    fs::write(f.local.join("local.txt"), "editado").unwrap();
    assert_eq!(f.run().await.unwrap().uploaded, 1);
    f.pair.propagate_deletions = true;
    fs::remove_file(f.local.join("local.txt")).unwrap();
    assert_eq!(f.run().await.unwrap().deleted, 1);
    assert!(!folder.join("local.txt").exists());
    // A replacement device with the same display name is never trusted.
    let mut replacement = device.clone();
    replacement["uid"] = "different-pc".into();
    fs::write(&devices, serde_json::to_vec(&vec![replacement]).unwrap()).unwrap();
    fs::write(f.local.join("private.txt"), "local only").unwrap();
    assert!(f.run().await.unwrap_err().contains("destino mudou"));
    assert!(!folder.join("private.txt").exists());
    // The virtual device collection and device roots cannot be trashed.
    assert!(f.cli.trash("/devices").await.is_err());
    assert!(f.cli.trash("/devices/Meu PC").await.is_err());
    // Missing/renamed computers do not cause local deletion propagation.
    fs::write(&devices, "[]").unwrap();
    assert!(f.run().await.is_err());
    assert!(f.local.join("remote.txt").exists());
}

#[tokio::test]
async fn missing_root_restores_instead_of_trashing_remote_and_resumes_normal_sync() {
    let mut f = Fixture::new(SyncMode::Bidirectional);
    f.pair.propagate_deletions = true;
    fs::create_dir(f.local.join("folder")).unwrap();
    fs::write(f.local.join("folder/one.txt"), "one").unwrap();
    fs::write(f.local.join("two.txt"), "two").unwrap();
    f.run().await.unwrap();
    fs::remove_dir_all(&f.local).unwrap();
    let result = f.run().await.unwrap();
    assert_eq!(result.deleted, 0);
    assert_eq!(result.uploaded, 0);
    assert!(result.conflicts.is_empty());
    assert_eq!(
        fs::read_to_string(f.local.join("folder/one.txt")).unwrap(),
        "one"
    );
    assert_eq!(fs::read_to_string(f.remote.join("two.txt")).unwrap(), "two");
    let journal: serde_json::Value =
        serde_json::from_slice(&fs::read(f.snapshot.with_extension("local-root.json")).unwrap())
            .unwrap();
    assert_eq!(journal["restoring"], false);
    fs::write(f.local.join("two.txt"), "new local edit").unwrap();
    assert_eq!(f.run().await.unwrap().uploaded, 1);
    fs::remove_file(f.local.join("two.txt")).unwrap();
    assert_eq!(f.run().await.unwrap().deleted, 1);
}

#[tokio::test]
async fn root_recovery_survives_failed_download_and_preserves_later_local_edits() {
    let mut f = Fixture::new(SyncMode::Bidirectional);
    f.pair.propagate_deletions = true;
    for name in ["one.txt", "two.txt"] {
        fs::write(f.local.join(name), name).unwrap();
    }
    f.run().await.unwrap();
    fs::remove_dir_all(&f.local).unwrap();
    fs::write(f._root.path().join("fail-transfer"), "fail").unwrap();
    assert!(f.run().await.is_err());
    assert!(f.local.is_dir());
    let snapshot = fs::read(&f.snapshot).unwrap();
    assert!(drive_core::local_root::prepare(&f.local, &f.snapshot)
        .unwrap()
        .restoring());
    fs::remove_file(f._root.path().join("fail-transfer")).unwrap();
    fs::write(f.local.join("one.txt"), "user edit during recovery").unwrap();
    let result = f.run().await.unwrap();
    assert_eq!(result.deleted, 0);
    assert_eq!(result.uploaded, 0);
    assert_eq!(result.conflicts, vec!["one.txt"]);
    assert_eq!(
        fs::read_to_string(f.local.join("two.txt")).unwrap(),
        "two.txt"
    );
    assert_eq!(
        fs::read_to_string(f.local.join("one.txt")).unwrap(),
        "user edit during recovery"
    );
    assert_eq!(
        fs::read_to_string(f.remote.join("one.txt")).unwrap(),
        "one.txt"
    );
    assert!(drive_core::local_root::prepare(&f.local, &f.snapshot)
        .unwrap()
        .restoring());
    assert!(!snapshot.is_empty());
}

#[tokio::test]
async fn missing_photo_and_album_roots_restore_known_photos_without_duplicates() {
    for source in ["/photos", "/albums"] {
        let mut f = Fixture::new(SyncMode::Download);
        f.pair.remote_path = source.into();
        f.photo("one", "IMG.jpg", "original");
        f.albums(serde_json::json!({"trip": {"name":"Trip", "photos":["one"]}}));
        assert_eq!(f.run().await.unwrap().downloaded, 1);
        fs::remove_dir_all(&f.local).unwrap();
        let report = f.run().await.unwrap();
        assert_eq!(report.downloaded, 1);
        assert_eq!(report.uploaded, 0);
        assert!(report.conflicts.is_empty());
        assert_eq!(f.run().await.unwrap().unchanged, 1);
        assert_eq!(f.cli.photos(None).await.unwrap().len(), 1);
    }
}
