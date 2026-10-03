use crate::{atomic_json, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::{io::Write, os::unix::fs::PermissionsExt, path::Path, time::Duration};

const MANIFEST_URL: &str = "https://proton.me/download/drive/cli/version.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub version: String,
    pub platform: String,
    pub url: String,
    pub sha512: String,
}

#[derive(Deserialize)]
struct Manifest {
    #[serde(rename = "Releases")]
    releases: Vec<ManifestRelease>,
}
#[derive(Deserialize)]
struct ManifestRelease {
    #[serde(rename = "CategoryName")]
    category: String,
    #[serde(rename = "Version")]
    version: String,
    #[serde(rename = "Files")]
    files: Vec<ManifestFile>,
}
#[derive(Deserialize)]
struct ManifestFile {
    #[serde(rename = "Platform")]
    platform: String,
    #[serde(rename = "Url")]
    url: String,
    #[serde(rename = "Sha512CheckSum")]
    sha512: String,
}

fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .user_agent("ProtonDriveDesktop/0.1.0")
        .build()
        .map_err(|e| e.to_string())
}

pub fn validate_release(release: &Release) -> Result<()> {
    semver::Version::parse(&release.version).map_err(|_| "Versão inválida.")?;
    let url = reqwest::Url::parse(&release.url).map_err(|_| "URL inválida.")?;
    if url.scheme() != "https"
        || url.host_str() != Some("proton.me")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || !url
            .path()
            .starts_with(&format!("/download/drive/cli/{}/", release.version))
    {
        return Err("Atualização fora da origem oficial da Proton.".into());
    }
    if release.sha512.len() != 128 || !release.sha512.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Checksum SHA-512 inválido.".into());
    }
    Ok(())
}

pub async fn latest() -> Result<Release> {
    let manifest: Manifest = client()?
        .get(MANIFEST_URL)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let platform = if cfg!(target_arch = "aarch64") {
        "linux/arm64"
    } else {
        "linux/x64-baseline"
    };
    let release = manifest
        .releases
        .into_iter()
        .filter(|r| r.category == "Stable")
        .filter_map(|r| semver::Version::parse(&r.version).ok().map(|v| (v, r)))
        .max_by(|a, b| a.0.cmp(&b.0))
        .ok_or("Nenhuma versão estável encontrada.")?
        .1;
    let file = release
        .files
        .into_iter()
        .find(|f| f.platform == platform)
        .ok_or("Plataforma indisponível.")?;
    let result = Release {
        version: release.version,
        platform: platform.into(),
        url: file.url,
        sha512: file.sha512.to_lowercase(),
    };
    validate_release(&result)?;
    Ok(result)
}

pub fn verify(bytes: &[u8], checksum: &str) -> Result<()> {
    if format!("{:x}", Sha512::digest(bytes)) != checksum.to_lowercase() {
        return Err(
            "SHA-512 não confere. Atualização rejeitada; a instalação anterior foi preservada."
                .into(),
        );
    }
    if !bytes.starts_with(b"\x7fELF") {
        return Err("O arquivo não é um executável Linux.".into());
    }
    Ok(())
}

pub async fn install(release: &Release, destination: &Path) -> Result<()> {
    validate_release(release)?;
    let mut response = client()?
        .get(&release.url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > 350 * 1024 * 1024 {
            return Err("Download excedeu o limite de tamanho.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    verify(&bytes, &release.sha512)?;
    let parent = destination.parent().ok_or("Diretório inválido.")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temporary.write_all(&bytes).map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    temporary
        .as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o755))
        .map_err(|e| e.to_string())?;
    // Close the write handle before exec (Linux ETXTBSY).
    let temporary = temporary.into_temp_path();
    let cli = crate::cli::Cli {
        binary: temporary.to_path_buf(),
    };
    let version = cli.run(&["version"], 30).await?;
    if !version.contains(&format!("@{}+", release.version)) {
        return Err("A versão executada diverge do manifesto.".into());
    }
    if destination.exists() {
        std::fs::copy(destination, destination.with_extension("previous"))
            .map_err(|e| e.to_string())?;
    }
    temporary.persist(destination).map_err(|e| e.to_string())?;
    atomic_json(&parent.join("release.json"), release)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_untrusted_downloads() {
        let mut r = Release {
            version: "0.8.0".into(),
            platform: "linux/x64".into(),
            url: "https://proton.me/download/drive/cli/0.8.0/linux-x64/proton-drive".into(),
            sha512: "a".repeat(128),
        };
        assert!(validate_release(&r).is_ok());
        for url in [
            "http://proton.me/download/drive/cli/0.8.0/x",
            "https://proton.me.evil.example/download/drive/cli/0.8.0/x",
            "https://proton.me/other/x",
            "https://proton.me:444/download/drive/cli/0.8.0/x",
        ] {
            r.url = url.into();
            assert!(validate_release(&r).is_err());
        }
    }
    #[test]
    fn rejects_tampered_or_non_executable_files() {
        let bytes = b"\x7fELFbinary";
        let checksum = format!("{:x}", Sha512::digest(bytes));
        assert!(verify(bytes, &checksum).is_ok());
        assert!(verify(b"changed", &checksum).is_err());
        let html = b"<html>";
        assert!(verify(html, &format!("{:x}", Sha512::digest(html))).is_err());
    }
}
