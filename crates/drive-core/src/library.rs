use crate::{
    cli::{validate_source, Cli},
    model::RemoteEntry,
    sync::{fingerprint, sha1_file},
    Result,
};
use std::{fs, path::Path};

/// Obtain metadata from the service; callers never supply trusted MIME/size/UID values.
pub async fn entry(cli: &Cli, path: &str) -> Result<RemoteEntry> {
    validate_source(path)?;
    if matches!(path, "/photos" | "/albums" | "/my-files") {
        return Err("Select a file.".into());
    }
    cli.info(path).await
}

pub fn document_url(entry: &RemoteEntry) -> Result<String> {
    if !entry.native_document {
        return Err("This file is not a Proton document.".into());
    }
    let (volume, link) = entry
        .uid
        .split_once('~')
        .ok_or("Invalid document identifier.")?;
    let kind = if entry.media_type == "application/vnd.proton.sheet" {
        "sheet"
    } else {
        "doc"
    };
    let mut url = reqwest::Url::parse(&format!("https://docs.proton.me/{kind}"))
        .map_err(|e| e.to_string())?;
    url.query_pairs_mut()
        .append_pair("mode", "open")
        .append_pair("volumeId", volume)
        .append_pair("linkId", link);
    Ok(url.into())
}

/// The destination must be a fresh private staging directory. Check the exact
/// output, digest and server revision before making downloaded bytes visible.
pub async fn download(
    cli: &Cli,
    entry: &RemoteEntry,
    destination: &Path,
) -> Result<std::path::PathBuf> {
    if entry.directory || entry.native_document {
        return Err("Open Proton documents in the integrated editor.".into());
    }
    validate_source(&entry.path)?;
    let folder = destination.to_str().ok_or("Invalid path.")?;
    if entry.kind == "photo" {
        cli.run(
            &[
                "photo",
                "download",
                "-c",
                "skip",
                &entry.path,
                folder,
                "--json",
            ],
            3600,
        )
        .await?;
    } else {
        cli.run(
            &[
                "filesystem",
                "download",
                "-f",
                "skip",
                "-d",
                "merge",
                &entry.path,
                folder,
                "--json",
            ],
            3600,
        )
        .await?;
    }
    let file = destination.join(&entry.name);
    let meta = fs::symlink_metadata(&file).map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() != entry.size {
        return Err("Incomplete or unexpected download.".into());
    }
    if let Some(expected) = &entry.sha1 {
        if !sha1_file(&file)?.eq_ignore_ascii_case(expected) {
            return Err("Downloaded content does not match Drive.".into());
        }
    }
    if cli.info(&entry.path).await?.revision != entry.revision {
        return Err("The file changed during download. Try again.".into());
    }
    let _ = fingerprint(&file)?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_editor_url_uses_verified_uid_and_escapes_query() {
        let entry = RemoteEntry {
            uid: "volume=~link_123".into(),
            native_document: true,
            media_type: "application/vnd.proton.sheet".into(),
            ..Default::default()
        };
        assert_eq!(
            document_url(&entry).unwrap(),
            "https://docs.proton.me/sheet?mode=open&volumeId=volume%3D&linkId=link_123"
        );
        assert!(document_url(&RemoteEntry::default()).is_err());
    }
}
