//! Local changes wake the existing sync engine; the timer is only a cloud/rescan fallback.
use crate::{
    model::{Config, SyncPair},
    sync::ignored,
};
use notify::{
    event::{AccessKind, AccessMode},
    Event, EventKind, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub const QUIET_PERIOD: Duration = Duration::from_secs(2);
const MAX_BATCH_DELAY: Duration = Duration::from_secs(30);

struct Pending {
    first: Instant,
    last: Instant,
    not_before: Instant,
}

/// One entry per pair bounds memory during large copies. Claims are removed
/// BEFORE a transfer; changes arriving while it runs remain for the next pass.
#[derive(Default)]
pub struct ChangeQueue {
    pending: Mutex<BTreeMap<String, Pending>>,
}
impl ChangeQueue {
    pub fn changed(&self, id: &str, at: Instant) {
        let mut pending = self.pending.lock().unwrap();
        let change = pending.entry(id.into()).or_insert(Pending {
            first: at,
            last: at,
            not_before: at,
        });
        change.last = at;
    }
    pub fn schedule(&self, id: &str, at: Instant) {
        let ready = at.checked_sub(QUIET_PERIOD).unwrap_or(at);
        self.pending
            .lock()
            .unwrap()
            .entry(id.into())
            .or_insert(Pending {
                first: ready,
                last: ready,
                not_before: at,
            });
    }
    pub fn periodic(&self, config: &Config, unix_now: u64, at: Instant) {
        if config.paused {
            return;
        }
        for pair in config.pairs.iter().filter(|pair| pair.enabled) {
            if pair
                .last_run
                .is_none_or(|last| unix_now.saturating_sub(last) >= pair.interval_minutes * 60)
            {
                self.schedule(&pair.id, at);
            }
        }
    }
    pub fn claim_ready(&self, config: &Config, at: Instant) -> Vec<String> {
        if config.paused {
            return vec![];
        }
        let mut pending = self.pending.lock().unwrap();
        let ids: Vec<_> = config
            .pairs
            .iter()
            .filter(|pair| pair.enabled)
            .filter_map(|pair| {
                pending
                    .get(&pair.id)
                    .filter(|change| {
                        at >= change.not_before
                            && (at.saturating_duration_since(change.last) >= QUIET_PERIOD
                                || at.saturating_duration_since(change.first) >= MAX_BATCH_DELAY)
                    })
                    .map(|_| pair.id.clone())
            })
            .collect();
        for id in &ids {
            pending.remove(id);
        }
        ids
    }
    pub fn retry(&self, ids: &[String], at: Instant, delay: Duration) {
        let mut pending = self.pending.lock().unwrap();
        for id in ids {
            let ready = at.checked_sub(QUIET_PERIOD).unwrap_or(at);
            let change = pending.entry(id.clone()).or_insert(Pending {
                first: ready,
                last: ready,
                not_before: at,
            });
            change.not_before = change.not_before.max(at + delay);
        }
    }
    pub fn has_pending(&self) -> bool {
        !self.pending.lock().unwrap().is_empty()
    }
    pub fn retain(&self, ids: &BTreeSet<String>) {
        self.pending
            .lock()
            .unwrap()
            .retain(|id, _| ids.contains(id));
    }
}

pub fn relevant(event: &Event, root: &Path) -> bool {
    if event.need_rescan() {
        return true;
    }
    let changed = matches!(
        event.kind,
        EventKind::Any
            | EventKind::Create(_)
            | EventKind::Modify(_)
            | EventKind::Remove(_)
            | EventKind::Access(AccessKind::Close(AccessMode::Write))
    );
    changed
        && event.paths.iter().any(|path| {
            path.strip_prefix(root).is_ok_and(|relative| {
                !relative.components().any(|part| match part {
                    Component::Normal(name) => ignored(&name.to_string_lossy()),
                    _ => true,
                })
            })
        })
}

struct WatchedRoot {
    path: PathBuf,
    identity: Option<(u64, u64)>,
    _handle: Option<Box<dyn Watcher + Send>>,
    retry_at: Instant,
    error: Option<String>,
}

#[derive(Default)]
pub struct LocalMonitor {
    pub queue: Arc<ChangeQueue>,
    roots: BTreeMap<String, WatchedRoot>,
    faults: Arc<Mutex<BTreeMap<String, String>>>,
    paused: bool,
}
impl LocalMonitor {
    /// Reconcile after configuration changes and root replacement/remount.
    /// An unavailable native watcher falls back to two-second metadata polling.
    pub fn reconcile(&mut self, config: &Config, at: Instant) -> Vec<(String, String)> {
        let enabled: BTreeSet<_> = config
            .pairs
            .iter()
            .filter(|pair| pair.enabled)
            .map(|pair| pair.id.clone())
            .collect();
        self.roots.retain(|id, _| enabled.contains(id));
        self.queue.retain(&enabled);
        if self.paused && !config.paused {
            for id in &enabled {
                self.queue.schedule(id, at);
            }
        }
        self.paused = config.paused;
        let faults = std::mem::take(&mut *self.faults.lock().unwrap());
        let mut notices = Vec::new();
        for pair in config.pairs.iter().filter(|pair| pair.enabled) {
            let root = PathBuf::from(&pair.local_path);
            let identity = fs::symlink_metadata(&root)
                .ok()
                .filter(|meta| meta.is_dir() && !meta.file_type().is_symlink())
                .map(|meta| (meta.dev(), meta.ino()));
            let previous = self.roots.get(&pair.id);
            let failed = faults.get(&pair.id);
            let replace = previous.is_none_or(|old| {
                old.path != root
                    || old.identity != identity
                    || (old._handle.is_none() && at >= old.retry_at)
            }) || failed.is_some();
            if !replace {
                continue;
            }
            let old_error = previous.and_then(|old| old.error.clone());
            // Drop before registering again, freeing native watch descriptors.
            self.roots.remove(&pair.id);
            let (handle, error) = if identity.is_none() {
                (None, Some("Pasta indisponível ou substituída por um link simbólico. O monitor será retomado quando a pasta voltar.".into()))
            } else {
                self.watch(pair, &root, failed.is_some())
            };
            if error != old_error {
                if let Some(message) = &error {
                    notices.push((
                        pair.id.clone(),
                        crate::i18n::message(
                            "{0}: {1}",
                            &[
                                serde_json::json!(pair.name.to_string()),
                                crate::i18n::nested(message.to_string()),
                            ],
                        ),
                    ));
                }
            }
            self.roots.insert(
                pair.id.clone(),
                WatchedRoot {
                    path: root,
                    identity,
                    _handle: handle,
                    retry_at: at + Duration::from_secs(30),
                    error,
                },
            );
            self.queue.schedule(&pair.id, at);
        }
        notices
    }
    fn handler(
        &self,
        pair: &SyncPair,
        root: &Path,
    ) -> impl FnMut(notify::Result<Event>) + Send + 'static {
        let id = pair.id.clone();
        let root = root.to_owned();
        let queue = self.queue.clone();
        let faults = self.faults.clone();
        move |result| match result {
            Ok(event) if relevant(&event, &root) => queue.changed(&id, Instant::now()),
            Err(error) => {
                queue.changed(&id, Instant::now());
                faults.lock().unwrap().insert(id.clone(), error.to_string());
            }
            _ => (),
        }
    }
    fn watch(
        &self,
        pair: &SyncPair,
        root: &Path,
        force_poll: bool,
    ) -> (Option<Box<dyn Watcher + Send>>, Option<String>) {
        let config = notify::Config::default()
            .with_follow_symlinks(false)
            .with_poll_interval(QUIET_PERIOD);
        if !force_poll {
            if let Ok(mut watcher) = RecommendedWatcher::new(self.handler(pair, root), config) {
                if watcher.watch(root, RecursiveMode::Recursive).is_ok() {
                    return (Some(Box::new(watcher)), None);
                }
            }
        }
        match PollWatcher::new(self.handler(pair, root), config).and_then(|mut watcher| {
            watcher.watch(root, RecursiveMode::Recursive)?;
            Ok(watcher)
        }) {
            Ok(watcher) => (Some(Box::new(watcher)), Some("Monitor nativo indisponível; alterações locais serão verificadas automaticamente a cada 2 segundos.".into())),
            Err(error) => (None, Some(crate::i18n::message("Não foi possível monitorar a pasta ({0}). A verificação periódica continua ativa.", &[crate::i18n::nested(error.to_string())]))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SyncMode;
    use notify::event::{CreateKind, DataChange, ModifyKind, RemoveKind, RenameMode};
    fn config() -> Config {
        Config {
            pairs: vec![SyncPair {
                id: "a".into(),
                name: "Test".into(),
                local_path: "/tmp/test".into(),
                remote_path: "/my-files".into(),
                device_uid: None,
                mode: SyncMode::Bidirectional,
                interval_minutes: 5,
                enabled: true,
                last_run: Some(1000),
                propagate_deletions: false,
            }],
            ..Config::default()
        }
    }
    #[test]
    fn coalesces_saves_without_waiting_for_the_five_minute_timer() {
        let queue = ChangeQueue::default();
        let c = config();
        let at = Instant::now();
        queue.changed("a", at);
        queue.changed("a", at + Duration::from_secs(1));
        queue.periodic(&c, 1001, at + Duration::from_secs(1));
        assert!(queue
            .claim_ready(&c, at + Duration::from_secs(2))
            .is_empty());
        assert_eq!(queue.claim_ready(&c, at + Duration::from_secs(3)), ["a"]);
        assert!(queue
            .claim_ready(&c, at + Duration::from_secs(4))
            .is_empty());
    }
    #[test]
    fn busy_transfer_changes_survive_the_previous_claim_and_retry() {
        let queue = ChangeQueue::default();
        let c = config();
        let at = Instant::now();
        queue.schedule("a", at);
        let first = queue.claim_ready(&c, at);
        queue.changed("a", at + Duration::from_secs(1));
        queue.retry(&first, at, Duration::from_secs(1));
        assert!(queue
            .claim_ready(&c, at + Duration::from_secs(2))
            .is_empty());
        assert_eq!(queue.claim_ready(&c, at + Duration::from_secs(3)), ["a"]);
        queue.retry(&first, at + Duration::from_secs(3), Duration::from_secs(30));
        queue.periodic(&c, 2000, at + Duration::from_secs(4));
        assert!(queue
            .claim_ready(&c, at + Duration::from_secs(4))
            .is_empty());
        assert_eq!(queue.claim_ready(&c, at + Duration::from_secs(33)), ["a"]);
    }
    #[test]
    fn pause_and_disabled_pairs_do_not_consume_changes() {
        let queue = ChangeQueue::default();
        let mut c = config();
        let at = Instant::now();
        queue.schedule("a", at);
        c.paused = true;
        assert!(queue.claim_ready(&c, at).is_empty());
        c.paused = false;
        c.pairs[0].enabled = false;
        assert!(queue.claim_ready(&c, at).is_empty());
        c.pairs[0].enabled = true;
        assert_eq!(queue.claim_ready(&c, at), ["a"]);
    }
    #[test]
    fn continuous_writes_have_a_bounded_batch_delay() {
        let queue = ChangeQueue::default();
        let c = config();
        let at = Instant::now();
        for second in 0..=30 {
            queue.changed("a", at + Duration::from_secs(second));
        }
        assert_eq!(queue.claim_ready(&c, at + Duration::from_secs(30)), ["a"]);
    }
    #[test]
    fn reads_backups_and_ignored_paths_never_trigger_a_sync_loop() {
        let root = Path::new("/tmp/pair");
        let write = EventKind::Modify(ModifyKind::Data(DataChange::Content));
        for path in [
            "/tmp/pair/.proton-drive-desktop/backups/file",
            "/tmp/pair/.git/index",
            "/tmp/pair/node_modules/vue/file",
            "/tmp/pair-other/file",
        ] {
            assert!(!relevant(&Event::new(write).add_path(path.into()), root));
        }
        for access in [
            AccessKind::Read,
            AccessKind::Open(AccessMode::Read),
            AccessKind::Close(AccessMode::Read),
        ] {
            assert!(!relevant(
                &Event::new(EventKind::Access(access)).add_path(root.join("file")),
                root
            ));
        }
        for kind in [
            write,
            EventKind::Create(CreateKind::Folder),
            EventKind::Remove(RemoveKind::File),
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
            EventKind::Access(AccessKind::Close(AccessMode::Write)),
        ] {
            assert!(relevant(
                &Event::new(kind).add_path(root.join("folder/file")),
                root
            ));
        }
    }
}
