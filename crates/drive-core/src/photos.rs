use crate::{
    atomic_json,
    cli::Cli,
    library,
    model::*,
    sync::{fingerprint, safe_join, scan_local, sha1_file, INTERNAL},
    Result,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Serialize, Deserialize)]
struct PhotoBaseline {
    identity: String,
    remote_path: String,
    revision: String,
    local: Fingerprint,
}
type History = BTreeMap<String, PhotoBaseline>;

/// The UID suffix disambiguates camera filenames and equal album names.
pub fn local_name(entry: &RemoteEntry) -> String {
    let hash = format!("{:x}", Sha256::digest(entry.uid.as_bytes()));
    let name: String = entry
        .name
        .chars()
        .scan(0, |len, ch| {
            *len += ch.len_utf8();
            (*len <= 160).then_some(ch)
        })
        .collect();
    if !entry.directory {
        if let Some((stem, ext)) = name.rsplit_once('.') {
            return format!("{stem} [{}].{ext}", &hash[..12]);
        }
    }
    format!("{name} [{}]", &hash[..12])
}
fn is_media(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "jpg"
                    | "jpeg"
                    | "png"
                    | "webp"
                    | "gif"
                    | "heic"
                    | "heif"
                    | "avif"
                    | "tif"
                    | "tiff"
                    | "raw"
                    | "dng"
                    | "mp4"
                    | "mov"
                    | "m4v"
                    | "webm"
                    | "avi"
                    | "mkv"
            )
        })
}

pub async fn synchronize_photos(
    cli: &Cli,
    pair: &SyncPair,
    state_file: &Path,
    cancel: &AtomicBool,
    restoring: bool,
    progress: impl Fn(&str, &str),
) -> Result<SyncReport> {
    if pair.propagate_deletions {
        return Err("Fotos e álbuns preservam exclusões; não habilite propagação neste tipo de sincronização.".into());
    }
    let all_albums = pair.remote_path == "/albums";
    if all_albums && pair.mode != SyncMode::Download {
        return Err("Todos os álbuns suportam cópia contínua do Drive para o PC. Selecione um álbum para enviar novas fotos.".into());
    }
    let root = Path::new(&pair.local_path);
    if !root.is_absolute() || !root.is_dir() {
        return Err("A pasta local não está disponível.".into());
    }
    safe_join(root, "")?;
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let mut history: History = match fs::read(state_file) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| e.to_string())?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(e) => return Err(e.to_string()),
    };
    let local = scan_local(&root)?;
    let mut remote = BTreeMap::new();
    let mut directories = Vec::new();
    progress("scan", "Carregando fotos e álbuns");
    let sources = if all_albums {
        cli.albums().await?
    } else {
        vec![]
    };
    let contexts: Vec<(String, String)> = if all_albums {
        sources
            .iter()
            .map(|album| (local_name(album), album.path.clone()))
            .collect()
    } else {
        vec![(String::new(), pair.remote_path.clone())]
    };
    for (folder, source) in contexts {
        if cancel.load(Ordering::Relaxed) {
            return Err("Sincronização interrompida.".into());
        }
        if !folder.is_empty() {
            directories.push(folder.clone());
        }
        let photos = cli
            .photos((source != "/photos").then_some(source.as_str()))
            .await?;
        for photo in photos {
            let identity = format!("{source}:{}", photo.uid);
            let generated = if folder.is_empty() {
                local_name(&photo)
            } else {
                format!("{folder}/{}", local_name(&photo))
            };
            let relative = history
                .iter()
                .find(|(_, old)| old.identity == identity)
                .map(|(key, _)| key.clone())
                .unwrap_or(generated);
            if remote.insert(relative, (identity, photo)).is_some() {
                return Err("Nomes locais duplicados na biblioteca.".into());
            }
        }
    }
    // Listing every source completes before making any local changes.
    for folder in directories {
        let path = safe_join(&root, &folder)?;
        fs::create_dir_all(path).map_err(|e| e.to_string())?;
    }
    let mut report = SyncReport::default();
    let internal = safe_join(&root, INTERNAL)?;
    fs::create_dir_all(&internal).map_err(|e| e.to_string())?;
    for (relative, (identity, entry)) in &remote {
        if cancel.load(Ordering::Relaxed) {
            return Err("Sincronização interrompida.".into());
        }
        let path = safe_join(&root, relative)?;
        let current = local.get(relative);
        let old = history.get(relative);
        if let Some(current) = current {
            if old.is_some_and(|old| old.local == *current && old.revision == entry.revision) {
                report.unchanged += 1;
                continue;
            }
            if !current.directory
                && entry.sha1.as_ref().is_some_and(|hash| {
                    sha1_file(&path).is_ok_and(|actual| actual.eq_ignore_ascii_case(hash))
                })
            {
                history.insert(
                    relative.clone(),
                    PhotoBaseline {
                        identity: identity.clone(),
                        remote_path: entry.path.clone(),
                        revision: entry.revision.clone(),
                        local: current.clone(),
                    },
                );
                atomic_json(state_file, &history)?;
                report.unchanged += 1;
                continue;
            }
            // Photo revisions cannot be overwritten through the CLI. Keep edited
            // originals; importing under a new name is an explicit new photo.
            report.conflicts.push(relative.clone());
            progress(
                "conflict",
                &crate::i18n::message(
                    "{0}: foto local alterada; salve com outro nome para enviar uma nova foto",
                    &[serde_json::json!(relative.to_string())],
                ),
            );
            continue;
        }
        if old.is_some() && !restoring {
            report.conflicts.push(relative.clone());
            progress(
                "conflict",
                &crate::i18n::message(
                    "{0}: remoção local preservada; original mantido no Proton",
                    &[serde_json::json!(relative.to_string())],
                ),
            );
            continue;
        }
        if pair.mode == SyncMode::Upload {
            continue;
        }
        progress("download", relative);
        let stage = tempfile::tempdir_in(&internal).map_err(|e| e.to_string())?;
        let downloaded = library::download(cli, entry, stage.path()).await?;
        let final_path = safe_join(&root, relative)?;
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        // Atomic create without replacement; protects files appearing during transfer.
        fs::hard_link(&downloaded, &final_path).map_err(|e| e.to_string())?;
        history.insert(
            relative.clone(),
            PhotoBaseline {
                identity: identity.clone(),
                remote_path: entry.path.clone(),
                revision: entry.revision.clone(),
                local: fingerprint(&final_path)?,
            },
        );
        atomic_json(state_file, &history)?;
        report.downloaded += 1;
    }
    let known: BTreeSet<_> = remote.keys().cloned().collect();
    for (relative, old) in &history {
        if !known.contains(relative) && local.contains_key(relative) {
            report.conflicts.push(relative.clone());
            progress(
                "conflict",
                &crate::i18n::message(
                    "{0}: não está mais nesta biblioteca; cópia local preservada ({1})",
                    &[
                        serde_json::json!(relative.to_string()),
                        serde_json::json!(old.remote_path.to_string()),
                    ],
                ),
            );
        }
    }
    if pair.mode != SyncMode::Download {
        for (relative, current) in &local {
            if current.directory
                || !is_media(relative)
                || history.contains_key(relative)
                || known.contains(relative)
            {
                continue;
            }
            if cancel.load(Ordering::Relaxed) {
                return Err("Sincronização interrompida.".into());
            }
            let path = safe_join(&root, relative)?;
            let stage = tempfile::tempdir_in(&internal).map_err(|e| e.to_string())?;
            let name = path.file_name().ok_or("Nome inválido.")?;
            let stable = stage.path().join(name);
            fs::copy(&path, &stable).map_err(|e| e.to_string())?;
            if fingerprint(&stable)? != *current {
                return Err(crate::i18n::message(
                    "{0} mudou durante a leitura.",
                    &[serde_json::json!(relative.to_string())],
                ));
            }
            progress("upload", relative);
            cli.run(
                &[
                    "photo",
                    "upload",
                    "-c",
                    "skip",
                    stable.to_str().ok_or("Caminho inválido")?,
                    "--json",
                ],
                3600,
            )
            .await?;
            let digest = sha1_file(&stable)?;
            let candidates: Vec<_> = cli
                .photos(None)
                .await?
                .into_iter()
                .filter(|entry| {
                    entry.name == name.to_string_lossy()
                        && entry
                            .sha1
                            .as_ref()
                            .is_some_and(|hash| hash.eq_ignore_ascii_case(&digest))
                })
                .collect();
            if candidates.len() != 1 {
                return Err(crate::i18n::message("{0}: não foi possível identificar uma única foto enviada; original preservado.", &[serde_json::json!(relative.to_string())]));
            }
            let entry = &candidates[0];
            if pair.remote_path.starts_with("/albums/") {
                cli.run(
                    &[
                        "album",
                        "add-photo",
                        &pair.remote_path,
                        &entry.path,
                        "--json",
                    ],
                    120,
                )
                .await?;
                if !cli
                    .photos(Some(&pair.remote_path))
                    .await?
                    .iter()
                    .any(|photo| photo.uid == entry.uid)
                {
                    return Err("O Proton não confirmou a inclusão no álbum.".into());
                }
            }
            history.insert(
                relative.clone(),
                PhotoBaseline {
                    identity: format!("{}:{}", pair.remote_path, entry.uid),
                    remote_path: entry.path.clone(),
                    revision: entry.revision.clone(),
                    local: current.clone(),
                },
            );
            atomic_json(state_file, &history)?;
            report.uploaded += 1;
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equal_camera_names_get_distinct_stable_local_paths() {
        let a = RemoteEntry {
            name: "IMG_001.jpg".into(),
            uid: "a".into(),
            ..Default::default()
        };
        let b = RemoteEntry {
            uid: "b".into(),
            ..a.clone()
        };
        assert_ne!(local_name(&a), local_name(&b));
        assert!(local_name(&a).ends_with(".jpg"));
    }
}
