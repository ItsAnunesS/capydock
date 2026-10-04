use crate::{
    atomic_json,
    cli::{validate_name, validate_remote, Cli},
    model::*,
    Result,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

pub const INTERNAL: &str = ".proton-drive-desktop";
pub fn ignored(name: &str) -> bool {
    matches!(name, INTERNAL | ".git" | "node_modules" | ".DS_Store")
}

pub fn safe_join(root: &Path, relative: &str) -> Result<PathBuf> {
    let mut result = root.to_path_buf();
    if fs::symlink_metadata(root)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("The root folder cannot be a symbolic link.".into());
    }
    for component in Path::new(relative).components() {
        let Component::Normal(name) = component else {
            return Err("Path is outside the synced folder.".into());
        };
        validate_name(name.to_str().ok_or("File name is not UTF-8.")?)?;
        result.push(name);
        match fs::symlink_metadata(&result) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err("Symbolic links are not synced.".into())
            }
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
            _ => (),
        }
    }
    Ok(result)
}

pub fn fingerprint(path: &Path) -> Result<Fingerprint> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if metadata.is_dir() {
        return Ok(Fingerprint {
            hash: String::new(),
            size: 0,
            directory: true,
        });
    }
    if !metadata.is_file() {
        return Err("Only regular files can be synced.".into());
    }
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let after = file.metadata().map_err(|e| e.to_string())?;
    if metadata.len() != after.len() || metadata.modified().ok() != after.modified().ok() {
        return Err("File changed while reading; try in the next cycle.".into());
    }
    Ok(Fingerprint {
        hash: format!("{:x}", hasher.finalize()),
        size: metadata.len(),
        directory: false,
    })
}

pub fn sha1_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut digest = sha1::Sha1::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        digest.update(&buf[..n]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

pub fn scan_local(root: &Path) -> Result<BTreeMap<String, Fingerprint>> {
    let mut result = BTreeMap::new();
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !ignored(&e.file_name().to_string_lossy()))
    {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.depth() == 0 || entry.file_type().is_symlink() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_str()
            .ok_or("Name is not UTF-8.")?
            .to_string();
        safe_join(root, &relative)?;
        result.insert(relative, fingerprint(entry.path())?);
    }
    Ok(result)
}

pub async fn scan_remote(
    cli: &Cli,
    root: &str,
    cancel: &AtomicBool,
) -> Result<BTreeMap<String, RemoteEntry>> {
    let mut result = BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    while let Some(path) = pending.pop() {
        if cancel.load(Ordering::Relaxed) {
            return Err("Sync interrupted between operations.".into());
        }
        for entry in cli.list(&path).await? {
            if ignored(&entry.name) {
                continue;
            }
            let relative = entry
                .path
                .strip_prefix(&format!("{root}/"))
                .ok_or("Invalid remote path.")?
                .to_owned();
            if relative.split('/').count() > 100 || result.len() >= 100_000 {
                return Err("Folder exceeds the limit of 100 levels or 100,000 items.".into());
            }
            if entry.directory {
                pending.push(entry.path.clone());
            }
            if result.insert(relative, entry).is_some() {
                return Err("Duplicate names in Drive; rename the files before syncing.".into());
            }
        }
    }
    Ok(result)
}

pub async fn synchronize(
    cli: &Cli,
    pair: &SyncPair,
    state_file: &Path,
    cancel: &AtomicBool,
    progress: impl Fn(&str, &str),
) -> Result<SyncReport> {
    if cancel.load(Ordering::Relaxed) {
        return Err(crate::operations::CANCELLED.into());
    }
    let local_root = crate::local_root::prepare(Path::new(&pair.local_path), state_file)?;
    let restoring = local_root.restoring();
    let mut effective = pair.clone();
    if restoring {
        effective.propagate_deletions = false;
        effective.mode = SyncMode::Download;
        progress("recovery", &crate::i18n::message("Recovering local folder: {0}. Restoring files from Drive without propagating deletions.", &[serde_json::json!(pair.local_path)]));
    }
    let result = if pair.remote_path == "/photos" || pair.remote_path.starts_with("/albums") {
        crate::photos::synchronize_photos(cli, &effective, state_file, cancel, restoring, &progress)
            .await
    } else {
        synchronize_files(cli, &effective, state_file, cancel, restoring, &progress).await
    };
    if result
        .as_ref()
        .is_ok_and(|report| report.conflicts.is_empty())
        && !cancel.load(Ordering::Relaxed)
    {
        local_root.finish()?;
        if restoring {
            progress(
                "recovery",
                "Local folder recovered. Automatic sync resumed.",
            );
        }
    }
    result
}

async fn synchronize_files(
    cli: &Cli,
    pair: &SyncPair,
    state_file: &Path,
    cancel: &AtomicBool,
    restoring: bool,
    progress: impl Fn(&str, &str),
) -> Result<SyncReport> {
    validate_remote(&pair.remote_path)?;
    cli.verify_computer_pair(pair).await?;
    let root = Path::new(&pair.local_path);
    if !root.is_absolute() || !root.is_dir() {
        return Err(
            "Local folder is unavailable. Reconnect the disk or select another folder.".into(),
        );
    }
    safe_join(root, "")?;
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    progress("scan", "Comparing local and remote files");
    let local = scan_local(&root)?;
    let remote = scan_remote(cli, &pair.remote_path, cancel).await?;
    let mut snapshot: Snapshot = match fs::read(state_file) {
        Ok(data) => serde_json::from_slice(&data).map_err(|e| {
            crate::i18n::message(
                "Invalid sync history: {0}",
                &[crate::i18n::nested(e.to_string())],
            )
        })?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(e) => return Err(e.to_string()),
    };
    let keys: BTreeSet<_> = local
        .keys()
        .chain(remote.keys())
        .chain(snapshot.keys())
        .cloned()
        .collect();
    let mut deletions = Vec::new();
    let mut blocked: Vec<String> = vec![];
    let mut report = SyncReport::default();
    for relative in keys {
        if cancel.load(Ordering::Relaxed) {
            return Err("Sync interrupted between operations.".into());
        }
        if blocked
            .iter()
            .any(|prefix| relative.starts_with(&format!("{prefix}/")))
        {
            continue;
        }
        let l = local.get(&relative);
        let r = remote.get(&relative);
        let path = safe_join(&root, &relative)?;
        let remote_path = format!("{}/{}", pair.remote_path, relative);
        if r.is_some_and(|entry| entry.native_document) {
            report.online_only += 1;
            progress(
                "online",
                &crate::i18n::message(
                    "{0}: available in the Proton editor; offline export is not supported by the CLI",
                    &[serde_json::json!(relative.to_string())],
                ),
            );
            continue;
        }
        let baseline = snapshot.get(&relative);
        if l.is_none() && r.is_none() {
            snapshot.remove(&relative);
            atomic_json(state_file, &snapshot)?;
            continue;
        }
        // Only propagate an observed removal when the surviving subtree still
        // exactly matches the last successful scan. New/edited descendants conflict.
        if pair.propagate_deletions && baseline.is_some() {
            let prefix = format!("{relative}/");
            let delete_remote = l.is_none()
                && r.is_some()
                && pair.mode != SyncMode::Download
                && remote
                    .iter()
                    .filter(|(key, _)| *key == &relative || key.starts_with(&prefix))
                    .all(|(key, entry)| {
                        !entry.native_document
                            && !local.contains_key(key)
                            && snapshot
                                .get(key)
                                .is_some_and(|old| old.remote == entry.revision)
                    });
            let delete_local = r.is_none()
                && l.is_some()
                && pair.mode != SyncMode::Upload
                && local
                    .iter()
                    .filter(|(key, _)| *key == &relative || key.starts_with(&prefix))
                    .all(|(key, value)| {
                        !remote.contains_key(key)
                            && snapshot.get(key).is_some_and(|old| old.local == *value)
                    });
            if delete_remote || delete_local {
                blocked.push(relative.clone());
                deletions.push((relative.clone(), delete_remote));
                continue;
            }
        }
        let mut action = if restoring && l.is_none() && r.is_some() {
            Action::Download
        } else {
            decide(l, r, baseline, pair.mode)
        };
        // Equal contents also settle a previously reported conflict without resetting history.
        if action == Action::Conflict {
            if let (Some(l), Some(r)) = (l, r) {
                if !l.directory
                    && !r.directory
                    && l.size == r.size
                    && r.sha1.as_ref().is_some_and(|hash| {
                        sha1_file(&path).is_ok_and(|actual| actual.eq_ignore_ascii_case(hash))
                    })
                {
                    action = Action::Unchanged;
                }
            }
        }
        match action {
            Action::Conflict => {
                blocked.push(relative.clone());
                report.conflicts.push(relative.clone());
                progress(
                    "conflict",
                    &crate::i18n::message(
                        "{0}: copies preserved; review changes on both sides",
                        &[serde_json::json!(relative.to_string())],
                    ),
                );
                continue;
            }
            Action::Ignore => continue,
            Action::Unchanged => {
                report.unchanged += 1;
                if let (Some(l), Some(r)) = (l, r) {
                    snapshot.insert(
                        relative.clone(),
                        Baseline {
                            local: l.clone(),
                            remote: r.revision.clone(),
                        },
                    );
                }
            }
            Action::Upload => {
                progress("upload", &relative);
                let expected = l.ok_or("Local file missing.")?;
                if fingerprint(&path)? != *expected {
                    return Err(crate::i18n::message(
                        "{0} changed during sync.",
                        &[serde_json::json!(relative.to_string())],
                    ));
                }
                let (parent, name) = remote_path.rsplit_once('/').ok_or("Invalid remote path.")?;
                if expected.directory {
                    cli.run(
                        &["filesystem", "create-folder", parent, name, "--json"],
                        120,
                    )
                    .await?;
                } else {
                    // Stage a stable copy; preserve remote versions on updates.
                    let stage = tempfile::tempdir().map_err(|e| e.to_string())?;
                    let staged = stage.path().join(name);
                    fs::copy(&path, &staged).map_err(|e| e.to_string())?;
                    if fingerprint(&staged)? != *expected {
                        return Err(crate::i18n::message(
                            "{0} changed while copying.",
                            &[serde_json::json!(relative.to_string())],
                        ));
                    }
                    if let Some(previous) = r {
                        if cli.info(&remote_path).await?.revision != previous.revision {
                            return Err(crate::i18n::message(
                                "{0} changed in Drive; sync again.",
                                &[serde_json::json!(relative.to_string())],
                            ));
                        }
                    }
                    let strategy = if r.is_some() {
                        "create-new-revision"
                    } else {
                        "skip"
                    };
                    cli.run(
                        &[
                            "filesystem",
                            "upload",
                            "-f",
                            strategy,
                            "-d",
                            "merge",
                            staged.to_str().ok_or("Invalid path.")?,
                            parent,
                            "--json",
                        ],
                        3600,
                    )
                    .await?;
                }
                let uploaded = cli.info(&remote_path).await?;
                if !expected.directory
                    && !uploaded.sha1.as_ref().is_some_and(|hash| {
                        sha1_file(&path).is_ok_and(|h| h.eq_ignore_ascii_case(hash))
                    })
                {
                    return Err(crate::i18n::message(
                        "{0}: uploaded content could not be confirmed; history preserved.",
                        &[serde_json::json!(relative.to_string())],
                    ));
                }
                snapshot.insert(
                    relative.clone(),
                    Baseline {
                        local: expected.clone(),
                        remote: uploaded.revision,
                    },
                );
                report.uploaded += 1;
            }
            Action::Download => {
                progress("download", &relative);
                let entry = r.ok_or("Remote file missing.")?;
                if entry.directory {
                    fs::create_dir(&path).map_err(|e| e.to_string())?;
                } else {
                    let internal = safe_join(&root, INTERNAL)?;
                    fs::create_dir_all(&internal).map_err(|e| e.to_string())?;
                    let stage = tempfile::tempdir_in(&internal).map_err(|e| e.to_string())?;
                    cli.run(
                        &[
                            "filesystem",
                            "download",
                            "-f",
                            "skip",
                            "-d",
                            "merge",
                            &remote_path,
                            stage.path().to_str().ok_or("Invalid path.")?,
                            "--json",
                        ],
                        3600,
                    )
                    .await?;
                    let staged = stage.path().join(&entry.name);
                    let downloaded = fingerprint(&staged)?;
                    if downloaded.directory
                        || downloaded.size != entry.size
                        || entry.sha1.as_ref().is_some_and(|expected| {
                            sha1_file(&staged)
                                .is_ok_and(|actual| !actual.eq_ignore_ascii_case(expected))
                        })
                    {
                        return Err(crate::i18n::message(
                            "{0}: incomplete download.",
                            &[serde_json::json!(relative.to_string())],
                        ));
                    }
                    if cli.info(&remote_path).await?.revision != entry.revision {
                        return Err(crate::i18n::message(
                            "{0} changed in Drive during download.",
                            &[serde_json::json!(relative.to_string())],
                        ));
                    }
                    let path = safe_join(&root, &relative)?;
                    let current = if path.exists() {
                        Some(fingerprint(&path)?)
                    } else {
                        None
                    };
                    if current.as_ref() != l {
                        return Err(crate::i18n::message(
                            "{0} changed locally; copy preserved.",
                            &[serde_json::json!(relative.to_string())],
                        ));
                    }
                    if l.is_some() {
                        let backup = safe_join(
                            &root,
                            &format!("{INTERNAL}/backups/{}", uuid::Uuid::new_v4()),
                        )?;
                        fs::create_dir_all(&backup).map_err(|e| e.to_string())?;
                        fs::copy(&path, backup.join(&entry.name)).map_err(|e| e.to_string())?;
                        atomic_json(&backup.join("origin.json"), &relative)?;
                    }
                    fs::rename(&staged, &path).map_err(|e| e.to_string())?;
                }
                snapshot.insert(
                    relative.clone(),
                    Baseline {
                        local: fingerprint(&path)?,
                        remote: entry.revision.clone(),
                    },
                );
                report.downloaded += 1;
            }
        }
        // Commit every completed file so interruption never advances unfinished work.
        atomic_json(state_file, &snapshot)?;
    }
    if !report.conflicts.is_empty() && !deletions.is_empty() {
        progress(
            "conflict",
            "Deletions deferred until conflicts are resolved.",
        );
        return Ok(report);
    }
    // Complete uploads/downloads first: a renamed file has a verified new copy
    // before the old path can move to trash. Never use permanent CLI deletion.
    for (relative, delete_remote) in deletions {
        if cancel.load(Ordering::Relaxed) {
            return Err("Sync interrupted between operations.".into());
        }
        let path = safe_join(&root, &relative)?;
        let remote_path = format!("{}/{}", pair.remote_path, relative);
        let prefix = format!("{relative}/");
        if delete_remote {
            if path.exists() {
                return Err(crate::i18n::message(
                    "{0} reappeared on PC; removal cancelled.",
                    &[serde_json::json!(relative.to_string())],
                ));
            }
            let expected = remote.get(&relative).ok_or("Remote file missing.")?;
            if cli.info(&remote_path).await?.revision != expected.revision {
                return Err(crate::i18n::message(
                    "{0} changed in Drive; removal cancelled.",
                    &[serde_json::json!(relative.to_string())],
                ));
            }
            if expected.directory {
                let mut pending = vec![remote_path.clone()];
                while let Some(parent) = pending.pop() {
                    for entry in cli.list(&parent).await? {
                        if ignored(&entry.name) {
                            return Err(crate::i18n::message(
                                "{0}: contains ignored files; automatic removal cancelled.",
                                &[serde_json::json!(relative.to_string())],
                            ));
                        }
                        if entry.directory {
                            pending.push(entry.path);
                        }
                    }
                }
                let current = scan_remote(cli, &remote_path, cancel).await?;
                let original: BTreeMap<_, _> = remote
                    .iter()
                    .filter_map(|(key, entry)| {
                        key.strip_prefix(&prefix)
                            .map(|key| (key.to_owned(), entry.revision.clone()))
                    })
                    .collect();
                if current
                    .iter()
                    .map(|(key, entry)| (key.clone(), entry.revision.clone()))
                    .collect::<BTreeMap<_, _>>()
                    != original
                {
                    return Err(crate::i18n::message(
                        "{0}: folder changed in Drive; removal cancelled.",
                        &[serde_json::json!(relative.to_string())],
                    ));
                }
            }
            cli.trash(&remote_path).await?;
        } else {
            // Re-list the parent: an API error is never interpreted as absence.
            let parent = remote_path.rsplit_once('/').ok_or("Invalid path")?.0;
            if cli
                .list(parent)
                .await?
                .iter()
                .any(|entry| entry.path == remote_path)
            {
                return Err(crate::i18n::message(
                    "{0} reappeared in Drive; removal cancelled.",
                    &[serde_json::json!(relative.to_string())],
                ));
            }
            let expected = local.get(&relative).ok_or("Local file missing.")?;
            if fingerprint(&path)? != *expected {
                return Err(crate::i18n::message(
                    "{0} changed on PC; removal cancelled.",
                    &[serde_json::json!(relative.to_string())],
                ));
            }
            if expected.directory {
                for entry in walkdir::WalkDir::new(&path)
                    .follow_links(false)
                    .min_depth(1)
                {
                    let entry = entry.map_err(|e| e.to_string())?;
                    if entry.file_type().is_symlink()
                        || ignored(&entry.file_name().to_string_lossy())
                    {
                        return Err(crate::i18n::message(
                            "{0}: contains ignored files; automatic removal cancelled.",
                            &[serde_json::json!(relative.to_string())],
                        ));
                    }
                }
                let current = scan_local(&path)?;
                let original: BTreeMap<_, _> = local
                    .iter()
                    .filter_map(|(key, value)| {
                        key.strip_prefix(&prefix)
                            .map(|key| (key.to_owned(), value.clone()))
                    })
                    .collect();
                if current != original {
                    return Err(crate::i18n::message(
                        "{0}: folder changed on PC; removal cancelled.",
                        &[serde_json::json!(relative.to_string())],
                    ));
                }
            }
            let backup = safe_join(&root, &format!("{INTERNAL}/trash/{}", uuid::Uuid::new_v4()))?;
            fs::create_dir_all(&backup).map_err(|e| e.to_string())?;
            atomic_json(&backup.join("origin.json"), &relative)?;
            fs::rename(&path, backup.join(path.file_name().ok_or("Invalid name")?))
                .map_err(|e| e.to_string())?;
        }
        snapshot.retain(|key, _| key != &relative && !key.starts_with(&prefix));
        atomic_json(state_file, &snapshot)?;
        report.deleted += 1;
        progress(
            "trash",
            &crate::i18n::message(
                "{0}: moved to recovery trash",
                &[serde_json::json!(relative.to_string())],
            ),
        );
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_symlinks_and_traversal() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("link")).unwrap();
        assert!(safe_join(root.path(), "link/secret").is_err());
        assert!(safe_join(root.path(), "../secret").is_err());
        assert!(safe_join(root.path(), "/etc/passwd").is_err());
        assert!(safe_join(root.path(), "normal/file.txt").is_ok());
    }
    #[test]
    fn scanner_skips_internal_data_and_symlinks() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("real.txt"), "hello").unwrap();
        fs::create_dir(root.path().join(INTERNAL)).unwrap();
        fs::write(root.path().join(INTERNAL).join("backup"), "secret").unwrap();
        std::os::unix::fs::symlink("/etc/passwd", root.path().join("link")).unwrap();
        let result = scan_local(root.path()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result["real.txt"].size, 5);
    }
}
