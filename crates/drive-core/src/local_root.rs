//! Recreate a missing sync root only on its verified local storage, and keep a
//! durable restore-only journal outside it so a retry never propagates loss.
use crate::{atomic_json, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::ErrorKind,
    os::unix::fs::MetadataExt,
    path::{Component, Path, PathBuf},
};

pub const UNAVAILABLE: &str = "Local folder unavailable. Recovery will retry automatically when the location is available again.";
#[derive(Clone, Serialize, Deserialize)]
struct Identity {
    device: u64,
    inode: u64,
}
impl Identity {
    fn read(path: &Path) -> Result<Self> {
        let meta = fs::symlink_metadata(path).map_err(|_| UNAVAILABLE)?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err(UNAVAILABLE.into());
        }
        Ok(Self {
            device: meta.dev(),
            inode: meta.ino(),
        })
    }
    fn matches(&self, other: &Self) -> bool {
        self.device == other.device && self.inode == other.inode
    }
}
#[derive(Serialize, Deserialize)]
struct Journal {
    path: PathBuf,
    anchor: PathBuf,
    anchor_identity: Identity,
    root_identity: Option<Identity>,
    restoring: bool,
}
pub struct LocalRoot {
    journal: Journal,
    state: PathBuf,
}
impl LocalRoot {
    pub fn restoring(&self) -> bool {
        self.journal.restoring
    }
    pub fn finish(mut self) -> Result<()> {
        if self.journal.restoring {
            self.validate()?;
            self.journal.restoring = false;
            atomic_json(&self.state, &self.journal)?;
        }
        Ok(())
    }
    fn validate(&self) -> Result<()> {
        if !self
            .journal
            .anchor_identity
            .matches(&Identity::read(&self.journal.anchor)?)
        {
            return Err(UNAVAILABLE.into());
        }
        validate_components(&self.journal.path)?;
        if let Some(expected) = &self.journal.root_identity {
            if !expected.matches(&Identity::read(&self.journal.path)?) {
                return Err(UNAVAILABLE.into());
            }
        }
        Ok(())
    }
}

fn validate_components(path: &Path) -> Result<()> {
    if !path.is_absolute() || path.parent().is_none() {
        return Err("Invalid local folder.".into());
    }
    let mut current = PathBuf::new();
    for part in path.components() {
        match part {
            Component::RootDir | Component::Normal(_) => current.push(part),
            _ => return Err("Invalid local folder.".into()),
        }
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err("Symbolic links are not synced.".into())
            }
            Ok(meta) if !meta.is_dir() => return Err(UNAVAILABLE.into()),
            Err(e) if e.kind() != ErrorKind::NotFound => return Err(UNAVAILABLE.into()),
            _ => (),
        }
    }
    Ok(())
}
fn unescape_mount(value: &str) -> PathBuf {
    value
        .replace("\\040", " ")
        .replace("\\011", "\t")
        .replace("\\134", "\\")
        .into()
}
fn mount_points() -> Result<Vec<PathBuf>> {
    let content = fs::read_to_string("/proc/self/mountinfo").map_err(|_| UNAVAILABLE)?;
    Ok(content
        .lines()
        .filter_map(|line| line.split_whitespace().nth(4))
        .map(unescape_mount)
        .collect())
}
fn configured_mounts() -> Result<Vec<PathBuf>> {
    let content = match fs::read_to_string("/etc/fstab") {
        Ok(content) => content,
        Err(e) if e.kind() == ErrorKind::NotFound => String::new(),
        Err(_) => return Err(UNAVAILABLE.into()),
    };
    Ok(content
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .filter_map(|line| line.split_whitespace().nth(1))
        .map(unescape_mount)
        .filter(|path| path.is_absolute())
        .collect())
}
fn anchor_for(
    path: &Path,
    home: Option<&Path>,
    mounts: &[PathBuf],
    configured: &[PathBuf],
) -> Result<PathBuf> {
    // A configured but absent mount is not a missing ordinary directory.
    if configured
        .iter()
        .any(|m| path.starts_with(m) && !mounts.contains(m))
    {
        return Err(UNAVAILABLE.into());
    }
    let mounted = mounts
        .iter()
        .filter(|m| path.starts_with(m))
        .max_by_key(|m| m.components().count());
    let external = ["/mnt", "/media", "/run/media"]
        .iter()
        .any(|base| path.starts_with(base));
    if external && mounted.is_none_or(|m| m == Path::new("/") || m == Path::new("/run")) {
        return Err(UNAVAILABLE.into());
    }
    if let Some(home) = home.filter(|home| path.starts_with(home) && path != *home) {
        validate_components(home)?;
        if mounted.is_some_and(|m| m.starts_with(home) && m != home) {
            return Ok(mounted.unwrap().clone());
        }
        Identity::read(home)?;
        return Ok(home.to_owned());
    }
    // On an established external pair, remember the actual mount. For other
    // existing roots outside Home, only their current parent is a safe anchor.
    if let Some(mount) = mounted.filter(|m| m.as_path() != Path::new("/")) {
        return Ok(mount.clone());
    }
    if path.is_dir() {
        return Ok(path.parent().ok_or(UNAVAILABLE)?.to_owned());
    }
    Err(UNAVAILABLE.into())
}

pub fn prepare(path: &Path, snapshot: &Path) -> Result<LocalRoot> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    prepare_with(
        path,
        snapshot,
        home.as_deref(),
        &mount_points()?,
        &configured_mounts()?,
    )
}
fn prepare_with(
    path: &Path,
    snapshot: &Path,
    home: Option<&Path>,
    mounts: &[PathBuf],
    configured: &[PathBuf],
) -> Result<LocalRoot> {
    validate_components(path)?;
    let state = snapshot.with_extension("local-root.json");
    let existing = match fs::symlink_metadata(path) {
        Ok(_) => Some(Identity::read(path)?),
        Err(e) if e.kind() == ErrorKind::NotFound => None,
        Err(_) => return Err(UNAVAILABLE.into()),
    };
    let mut changed = false;
    let mut journal: Journal = match fs::read(&state) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|_| "Invalid recovery record; history preserved.".to_string())?,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            let anchor = anchor_for(path, home, mounts, configured)?;
            changed = true;
            Journal {
                path: path.into(),
                anchor_identity: Identity::read(&anchor)?,
                anchor,
                root_identity: existing.clone(),
                restoring: existing.is_none(),
            }
        }
        Err(e) => return Err(e.to_string()),
    };
    if journal.path != path
        || !path.starts_with(&journal.anchor)
        || !journal
            .anchor_identity
            .matches(&Identity::read(&journal.anchor)?)
    {
        return Err(UNAVAILABLE.into());
    }
    if configured
        .iter()
        .any(|m| path.starts_with(m) && !mounts.contains(m))
    {
        return Err(UNAVAILABLE.into());
    }
    if existing
        .as_ref()
        .is_some_and(|id| id.device != journal.anchor_identity.device)
    {
        return Err(UNAVAILABLE.into());
    }
    if existing.as_ref().is_none_or(|id| {
        journal
            .root_identity
            .as_ref()
            .is_none_or(|old| !old.matches(id))
    }) {
        journal.restoring = true;
        changed = true;
    }
    // Commit BEFORE mkdir: a crash, cancellation or failed download must retain
    // restore-only mode even when the new empty directory survives a restart.
    if changed {
        atomic_json(&state, &journal)?;
    }
    if existing.is_none() {
        let relative = path
            .strip_prefix(&journal.anchor)
            .map_err(|_| UNAVAILABLE)?;
        let mut current = journal.anchor.clone();
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Err(UNAVAILABLE.into());
            };
            current.push(name);
            match fs::create_dir(&current) {
                Ok(()) => (),
                Err(e) if e.kind() == ErrorKind::AlreadyExists => (),
                Err(_) => return Err(UNAVAILABLE.into()),
            }
            if Identity::read(&current)?.device != journal.anchor_identity.device {
                return Err(UNAVAILABLE.into());
            }
        }
    }
    if changed {
        journal.root_identity = Some(Identity::read(path)?);
        atomic_json(&state, &journal)?;
    }
    let result = LocalRoot { journal, state };
    result.validate()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_missing_home_library_recovers_and_journal_survives_restart() {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("Documents/Proton/Arquivos");
        let state = home.path().join("snapshots/pair.json");
        let prepared = prepare_with(&root, &state, Some(home.path()), &["/".into()], &[]).unwrap();
        assert!(prepared.restoring());
        assert!(root.is_dir());
        drop(prepared);
        assert!(
            prepare_with(&root, &state, Some(home.path()), &["/".into()], &[])
                .unwrap()
                .restoring()
        );
        prepare_with(&root, &state, Some(home.path()), &["/".into()], &[])
            .unwrap()
            .finish()
            .unwrap();
        assert!(
            !prepare_with(&root, &state, Some(home.path()), &["/".into()], &[])
                .unwrap()
                .restoring()
        );
    }
    #[test]
    fn unavailable_mount_wrong_anchor_symlinks_and_files_are_never_recreated() {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("disk/folder");
        let state = home.path().join("s.json");
        let mount = home.path().join("disk");
        assert!(prepare_with(
            &root,
            &state,
            Some(home.path()),
            &["/".into()],
            std::slice::from_ref(&mount)
        )
        .is_err());
        assert!(!mount.exists());
        fs::create_dir(&mount).unwrap();
        let prepared = prepare_with(
            &root,
            &state,
            Some(home.path()),
            std::slice::from_ref(&mount),
            &[],
        )
        .unwrap();
        prepared.finish().unwrap();
        fs::rename(&mount, home.path().join("old-disk")).unwrap();
        fs::create_dir(&mount).unwrap();
        assert!(prepare_with(
            &root,
            &state,
            Some(home.path()),
            std::slice::from_ref(&mount),
            &[]
        )
        .is_err());
        assert!(!root.exists());
        fs::write(mount.join("file"), b"original").unwrap();
        assert!(prepare_with(&mount.join("file"), &state, Some(home.path()), &[], &[]).is_err());
        std::os::unix::fs::symlink(home.path().join("old-disk"), mount.join("link")).unwrap();
        assert!(prepare_with(
            &mount.join("link/nested"),
            &state,
            Some(home.path()),
            &[],
            &[]
        )
        .is_err());
    }
}
