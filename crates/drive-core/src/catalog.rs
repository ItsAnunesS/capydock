//! A display-only, account-scoped metadata cache. Never used as sync truth.
use crate::{model::RemoteEntry, now, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

const MAX_DISK_BYTES: u64 = 32 * 1024 * 1024;
const MAX_VIEWS: usize = 128;
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Cached {
    entries: Vec<RemoteEntry>,
    updated_at: Option<u64>,
    partial: bool,
    #[serde(skip)]
    refreshing: bool,
    #[serde(skip)]
    error: Option<String>,
    #[serde(skip)]
    folders_done: usize,
    #[serde(skip)]
    folders_pending: usize,
    #[serde(skip)]
    revision: u64,
    #[serde(skip)]
    run: u64,
    #[serde(skip)]
    attempted_at: u64,
    #[serde(skip)]
    invalidated: bool,
    #[serde(skip)]
    touched: u64,
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Disk {
    schema: u8,
    account: Option<String>,
    views: BTreeMap<String, Cached>,
}
struct Data {
    disk: Disk,
    active: bool,
    generation: u64,
    sequence: u64,
}
#[derive(Clone)]
pub struct Catalog {
    data: Arc<Mutex<Data>>,
    file: PathBuf,
    persistence: Arc<Mutex<()>>,
}
#[derive(Clone)]
pub struct Ticket {
    pub path: String,
    generation: u64,
    run: u64,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryView {
    /// None means the caller already has this revision. Avoid repeatedly sending
    /// a large document index across IPC while only the progress counter changes.
    pub entries: Option<Vec<RemoteEntry>>,
    pub revision: u64,
    pub updated_at: Option<u64>,
    pub refreshing: bool,
    pub partial: bool,
    pub folders_done: usize,
    pub folders_pending: usize,
    pub error: Option<String>,
}
impl Catalog {
    pub fn open(file: PathBuf) -> Self {
        let disk = fs::metadata(&file)
            .ok()
            .filter(|m| m.len() <= MAX_DISK_BYTES)
            .and_then(|_| fs::read(&file).ok())
            .and_then(|bytes| serde_json::from_slice::<Disk>(&bytes).ok())
            .filter(|d| d.schema == 1 && d.views.len() <= MAX_VIEWS)
            .unwrap_or_default();
        Self {
            persistence: Arc::new(Mutex::new(())),
            data: Arc::new(Mutex::new(Data {
                disk,
                active: false,
                generation: 0,
                sequence: 1,
            })),
            file,
        }
    }
    pub fn activate(&self, account: &str) {
        let mut data = self.data.lock().unwrap();
        if data.disk.account.as_deref() != Some(account) {
            data.generation += 1;
            data.disk = Disk {
                schema: 1,
                account: Some(account.into()),
                views: BTreeMap::new(),
            };
        }
        data.active = true;
    }
    pub fn deactivate(&self) {
        let mut data = self.data.lock().unwrap();
        data.active = false;
        data.generation += 1;
        for view in data.disk.views.values_mut() {
            view.refreshing = false;
        }
    }
    pub fn invalidate(&self) {
        let mut data = self.data.lock().unwrap();
        for view in data.disk.views.values_mut() {
            view.invalidated = true;
        }
    }
    pub fn begin(&self, path: &str, force: bool) -> Option<Ticket> {
        let mut data = self.data.lock().unwrap();
        if !data.active {
            return None;
        }
        if !data.disk.views.contains_key(path) && data.disk.views.len() >= MAX_VIEWS {
            let oldest = data
                .disk
                .views
                .iter()
                .filter(|(key, v)| !v.refreshing && key.as_str() != "/documents")
                .min_by_key(|(_, v)| v.touched)
                .map(|(key, _)| key.clone());
            data.disk.views.remove(&oldest?);
        }
        let at = now();
        let ttl = if path == "/documents" { 300 } else { 60 };
        let generation = data.generation;
        data.sequence += 1;
        let run = data.sequence;
        let view = data
            .disk
            .views
            .entry(path.into())
            .or_insert_with(|| Cached {
                partial: true,
                ..Cached::default()
            });
        view.touched = at;
        if view.refreshing
            || (!force
                && (view.error.as_deref() == Some(crate::operations::CANCELLED)
                    || (view.error.is_some() && at.saturating_sub(view.attempted_at) < 30)
                    || (!view.invalidated
                        && !view.partial
                        && view.updated_at.is_some_and(|t| at.saturating_sub(t) < ttl))))
        {
            return None;
        }
        view.refreshing = true;
        view.error = None;
        view.folders_done = 0;
        view.folders_pending = 1;
        view.attempted_at = at;
        view.run = run;
        Some(Ticket {
            path: path.into(),
            generation,
            run,
        })
    }
    pub fn valid(&self, ticket: &Ticket) -> bool {
        let data = self.data.lock().unwrap();
        data.active
            && data.generation == ticket.generation
            && data
                .disk
                .views
                .get(&ticket.path)
                .is_some_and(|v| v.run == ticket.run && v.refreshing)
    }
    pub fn snapshot(&self, path: &str, revision: Option<u64>) -> LibraryView {
        let data = self.data.lock().unwrap();
        let empty = Cached {
            partial: true,
            ..Cached::default()
        };
        let view = if data.active {
            data.disk.views.get(path).unwrap_or(&empty)
        } else {
            &empty
        };
        LibraryView {
            entries: if revision == Some(view.revision) {
                None
            } else {
                Some(view.entries.clone())
            },
            revision: view.revision,
            updated_at: view.updated_at,
            refreshing: view.refreshing,
            partial: view.partial,
            folders_done: view.folders_done,
            folders_pending: view.folders_pending,
            error: view.error.clone(),
        }
    }
    pub fn publish(
        &self,
        ticket: &Ticket,
        entries: Vec<RemoteEntry>,
        done: usize,
        pending: usize,
        complete: bool,
    ) {
        let mut data = self.data.lock().unwrap();
        if !data.active || data.generation != ticket.generation {
            return;
        }
        data.sequence += 1;
        let sequence = data.sequence;
        let Some(view) = data
            .disk
            .views
            .get_mut(&ticket.path)
            .filter(|v| v.run == ticket.run && v.refreshing)
        else {
            return;
        };
        if view.entries != entries {
            view.entries = entries;
            view.revision = sequence;
        }
        view.folders_done = done;
        view.folders_pending = pending;
        view.partial = !complete;
        if complete {
            view.updated_at = Some(now());
            view.refreshing = false;
            view.error = None;
            view.invalidated = false;
        }
    }
    pub fn fail(&self, ticket: &Ticket, error: String) {
        let mut data = self.data.lock().unwrap();
        if !data.active || data.generation != ticket.generation {
            return;
        }
        if let Some(view) = data
            .disk
            .views
            .get_mut(&ticket.path)
            .filter(|v| v.run == ticket.run)
        {
            view.refreshing = false;
            view.error = Some(error);
        }
    }
    pub fn persist(&self) -> Result<()> {
        use std::io::Write;
        // Serialize and fsync outside the snapshot lock; keep writers ordered.
        let _writer = self.persistence.lock().unwrap();
        let disk = {
            let data = self.data.lock().unwrap();
            if !data.active {
                return Ok(());
            }
            data.disk.clone()
        };
        let bytes = serde_json::to_vec(&disk).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_DISK_BYTES {
            return Ok(());
        }
        let parent = self.file.parent().ok_or("Caminho inválido")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        file.persist(&self.file).map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub fn is_document(entry: &RemoteEntry) -> bool {
    !entry.directory
        && (entry.native_document
            || entry.media_type == "application/pdf"
            || entry.media_type.starts_with("text/")
            || [
                "doc", "docx", "odt", "xls", "xlsx", "ods", "ppt", "pptx", "pdf", "txt", "md",
                "csv",
            ]
            .iter()
            .any(|ext| entry.name.to_lowercase().ends_with(&format!(".{ext}"))))
}

#[derive(Clone, Serialize)]
pub struct FolderRequest {
    pub path: String,
    pub uid: Option<String>,
}
pub async fn read_batch(
    helper: &crate::cli::Cli,
    parents: &[FolderRequest],
) -> Result<Vec<(String, Vec<RemoteEntry>)>> {
    if parents.is_empty() || parents.len() > 4 {
        return Err("Lote de metadados inválido.".into());
    }
    let requests = serde_json::to_string(parents).map_err(|e| e.to_string())?;
    let output = helper
        .run(&["library", "list-batch", &requests, "--json"], 120)
        .await?;
    #[derive(Deserialize)]
    struct Page {
        path: String,
        entries: Vec<serde_json::Value>,
    }
    let pages: Vec<Page> = serde_json::from_str(&output).map_err(|e| e.to_string())?;
    if pages.len() != parents.len() {
        return Err("Lote de metadados inválido.".into());
    }
    pages
        .into_iter()
        .zip(parents)
        .map(|(page, parent)| {
            if page.path != parent.path {
                return Err("Caminho remoto inválido.".into());
            }
            let entries = page
                .entries
                .iter()
                .map(|node| crate::cli::parse_entry(node, &page.path))
                .collect::<Result<Vec<_>>>()?;
            Ok((page.path, entries))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn doc(name: &str) -> RemoteEntry {
        RemoteEntry {
            name: name.into(),
            path: format!("/my-files/{name}"),
            ..RemoteEntry::default()
        }
    }
    #[test]
    fn cache_is_immediate_single_flight_and_keeps_last_good_data_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Catalog::open(dir.path().join("cache.json"));
        cache.activate("a");
        let ticket = cache.begin("/documents", false).unwrap();
        assert!(cache.begin("/documents", true).is_none());
        cache.publish(&ticket, vec![doc("A.pdf")], 1, 0, true);
        assert!(cache.begin("/documents", false).is_none());
        let view = cache.snapshot("/documents", None);
        assert_eq!(view.entries.unwrap().len(), 1);
        assert!(cache
            .snapshot("/documents", Some(view.revision))
            .entries
            .is_none());
        let ticket = cache.begin("/documents", true).unwrap();
        cache.fail(&ticket, "network".into());
        let view = cache.snapshot("/documents", None);
        assert_eq!(view.entries.unwrap()[0].name, "A.pdf");
        assert!(!view.refreshing);
        assert!(cache.begin("/documents", false).is_none());
    }
    #[test]
    fn persisted_metadata_is_hidden_until_account_verified_and_old_work_cannot_publish() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("cache.json");
        let cache = Catalog::open(file.clone());
        cache.activate("a");
        let ticket = cache.begin("/documents", false).unwrap();
        cache.publish(&ticket, vec![doc("private.pdf")], 1, 0, true);
        cache.persist().unwrap();
        let restored = Catalog::open(file);
        assert!(restored
            .snapshot("/documents", None)
            .entries
            .unwrap()
            .is_empty());
        restored.activate("a");
        assert_eq!(
            restored.snapshot("/documents", None).entries.unwrap().len(),
            1
        );
        let stale = restored.begin("/documents", true).unwrap();
        restored.deactivate();
        restored.activate("b");
        restored.publish(&stale, vec![doc("private.pdf")], 1, 0, true);
        assert!(restored
            .snapshot("/documents", None)
            .entries
            .unwrap()
            .is_empty());
        assert!(!restored.valid(&stale));
    }
    #[test]
    fn partial_results_and_forced_refresh_do_not_disappear() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Catalog::open(dir.path().join("cache.json"));
        cache.activate("a");
        let ticket = cache.begin("/documents", false).unwrap();
        cache.publish(&ticket, vec![doc("first.pdf")], 2, 20, false);
        let view = cache.snapshot("/documents", None);
        assert!(view.refreshing && view.partial);
        assert_eq!(view.folders_done, 2);
        assert_eq!(view.folders_pending, 20);
        cache.publish(
            &ticket,
            vec![doc("first.pdf"), doc("second.pdf")],
            22,
            0,
            true,
        );
        cache.invalidate();
        assert!(cache.begin("/documents", false).is_some());
        assert_eq!(cache.snapshot("/documents", None).entries.unwrap().len(), 2);
    }
}
