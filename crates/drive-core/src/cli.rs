use crate::{model::RemoteEntry, Result};
use serde_json::Value;
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::process::Command;

// One account process at a time: the official CLI stores refreshed credentials
// per process. Release between calls (not between whole operations), allowing
// navigation to interleave with transfers without racing session refreshes.
static PROCESS_GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Clone)]
pub struct Cli {
    pub binary: PathBuf,
}
impl Cli {
    pub async fn account(&self) -> Result<(String, Option<String>)> {
        let raw = self
            .run(&["filesystem", "info", "/my-files", "--json"], 90)
            .await?;
        let value: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let node = value.get("value").unwrap_or(&value);
        let uid = node["uid"]
            .as_str()
            .ok_or("Não foi possível identificar a conta.")?
            .to_owned();
        let email = node["ownedBy"]["email"].as_str().map(str::to_owned);
        Ok((uid, email))
    }
    pub async fn run(&self, args: &[&str], seconds: u64) -> Result<String> {
        let _permit = PROCESS_GATE.lock().await;
        let mut command = Command::new(&self.binary);
        command
            .args(args)
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .env("NO_COLOR", "1")
            .env("PROTON_DRIVE_LOG_LEVEL", "WARNING");
        let output = tokio::time::timeout(Duration::from_secs(seconds), command.output())
            .await
            .map_err(|_| "O CLI excedeu o tempo limite. Tente novamente.".to_string())?
            .map_err(|e| {
                crate::i18n::message(
                    "Não foi possível executar o CLI: {0}",
                    &[crate::i18n::nested(e.to_string())],
                )
            })?;
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        if !output.status.success() {
            if args.first() == Some(&"auth") {
                return Err("O login ou logout não foi concluído. Verifique o navegador e se o cofre de credenciais do Linux está desbloqueado.".into());
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            let text: &str = if stderr.trim().is_empty() {
                &stdout
            } else {
                &stderr
            };
            return Err(crate::i18n::message(
                "Falha no CLI: {0}",
                &[crate::i18n::nested(
                    text.chars().take(1500).collect::<String>().trim(),
                )],
            ));
        }
        Ok(stdout)
    }
    pub async fn list(&self, path: &str) -> Result<Vec<RemoteEntry>> {
        if path == "/devices" {
            return self.computers().await;
        }
        validate_remote(path)?;
        let output = self
            .run(&["filesystem", "list", path, "--json"], 120)
            .await?;
        let nodes: Vec<Value> = serde_json::from_str(&output).map_err(|e| {
            crate::i18n::message(
                "Resposta JSON inválida do CLI: {0}",
                &[crate::i18n::nested(e.to_string())],
            )
        })?;
        nodes.iter().map(|node| parse_entry(node, path)).collect()
    }
    pub async fn albums(&self) -> Result<Vec<RemoteEntry>> {
        self.photo_list(&["album", "list", "--json"], "/albums")
            .await
    }
    pub async fn photos(&self, album: Option<&str>) -> Result<Vec<RemoteEntry>> {
        if let Some(path) = album {
            validate_source(path)?;
            if !path.starts_with("/albums/") {
                return Err("Álbum inválido.".into());
            }
            self.photo_list(
                &["album", "photos", "--load-details", path, "--json"],
                "/photos",
            )
            .await
        } else {
            self.photo_list(
                &["photo", "timeline", "--load-details", "--json"],
                "/photos",
            )
            .await
        }
    }
    async fn photo_list(&self, args: &[&str], parent: &str) -> Result<Vec<RemoteEntry>> {
        let output = self.run(args, 600).await?;
        let nodes: Vec<Value> = serde_json::from_str(&output).map_err(|e| {
            crate::i18n::message(
                "Metadados de fotos inválidos: {0}",
                &[crate::i18n::nested(e.to_string())],
            )
        })?;
        nodes
            .iter()
            .map(|node| {
                let mut entry = parse_entry(node, parent)?;
                validate_name(&entry.uid)?;
                entry.path = format!("{parent}/{}", entry.uid);
                Ok(entry)
            })
            .collect()
    }
    pub async fn trash(&self, path: &str) -> Result<()> {
        validate_remote(path)?;
        if path == "/my-files" || (path.starts_with("/devices/") && path.split('/').count() == 3) {
            return Err("Não é possível remover a raiz.".into());
        }
        let output = self
            .run(&["filesystem", "trash", path, "--json"], 120)
            .await?;
        let results: Vec<Value> = serde_json::from_str(&output).map_err(|e| e.to_string())?;
        if results.is_empty()
            || results
                .iter()
                .any(|item| item["ok"].as_bool() != Some(true))
        {
            return Err("O Drive não confirmou a remoção para a lixeira.".into());
        }
        Ok(())
    }
    pub async fn folders(&self, path: &str) -> Result<Vec<RemoteEntry>> {
        if path == "/devices" {
            return self.computers().await;
        }
        validate_remote(path)?;
        let output = self
            .run(
                &["filesystem", "list", "--type", "folder", path, "--json"],
                120,
            )
            .await?;
        let nodes: Vec<Value> = serde_json::from_str(&output).map_err(|e| {
            crate::i18n::message(
                "Resposta JSON inválida do CLI: {0}",
                &[crate::i18n::nested(e.to_string())],
            )
        })?;
        nodes.iter().map(|node| parse_entry(node, path)).collect()
    }
    pub async fn info(&self, path: &str) -> Result<RemoteEntry> {
        validate_source(path)?;
        let output = self
            .run(&["filesystem", "info", path, "--json"], 120)
            .await?;
        let node = serde_json::from_str(&output).map_err(|e| {
            crate::i18n::message(
                "Metadados inválidos: {0}",
                &[crate::i18n::nested(e.to_string())],
            )
        })?;
        let mut entry = parse_entry(
            &node,
            path.rsplit_once('/').map(|x| x.0).unwrap_or("/my-files"),
        )?;
        entry.path = path.to_string();
        Ok(entry)
    }
}

pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', '\0'])
        || name.chars().any(char::is_control)
    {
        return Err("Nome de arquivo incompatível com a sincronização local.".into());
    }
    Ok(())
}

pub fn validate_remote(path: &str) -> Result<()> {
    if path != "/my-files" && !path.starts_with("/my-files/") && !path.starts_with("/devices/") {
        return Err("Selecione uma pasta em Meus arquivos ou em um computador registrado.".into());
    }
    for part in path.trim_start_matches('/').split('/') {
        validate_name(part)?;
    }
    Ok(())
}

pub fn validate_source(path: &str) -> Result<()> {
    if path == "/photos" || path == "/albums" {
        return Ok(());
    }
    if let Some(uid) = path
        .strip_prefix("/photos/")
        .or_else(|| path.strip_prefix("/albums/"))
    {
        return validate_name(uid);
    }
    validate_remote(path)
}

pub fn parse_entry(node: &Value, parent: &str) -> Result<RemoteEntry> {
    if node.get("ok").and_then(Value::as_bool) == Some(false)
        || node
            .get("errors")
            .and_then(Value::as_array)
            .is_some_and(|e| !e.is_empty())
    {
        return Err("O Proton Drive retornou metadados que não puderam ser verificados.".into());
    }
    let node =
        if node.get("ok").and_then(Value::as_bool) == Some(true) && node.get("value").is_some() {
            &node["value"]
        } else {
            node
        };
    let name = node["name"]
        .as_str()
        .or_else(|| {
            if node["name"]["ok"].as_bool() == Some(true) {
                node["name"]["value"].as_str()
            } else {
                None
            }
        })
        .ok_or("Não foi possível decifrar o nome de um arquivo.")?;
    validate_name(name)?;
    let directory = match node["type"].as_str() {
        Some("folder" | "album") => true,
        Some("file" | "photo") => false,
        _ => return Err("Tipo de arquivo não suportado.".into()),
    };
    let media_type = node["mediaType"].as_str().unwrap_or("");
    let native_document = media_type.contains("proton");
    let uid = node["uid"].as_str().ok_or("Arquivo sem identificador.")?;
    let rev = if directory || native_document {
        uid
    } else {
        node["activeRevision"]["uid"]
            .as_str()
            .ok_or("Arquivo sem revisão verificável.")?
    };
    Ok(RemoteEntry {
        uid: uid.into(),
        media_type: media_type.into(),
        kind: node["type"].as_str().unwrap_or("file").into(),
        native_document,
        photo_count: node["album"]["photoCount"].as_u64().unwrap_or(0),
        modified: node["modificationTime"].as_str().map(str::to_owned),
        name: name.into(),
        path: format!("{parent}/{name}"),
        directory,
        revision: format!("{uid}:{rev}"),
        size: node["activeRevision"]["claimedSize"].as_u64().unwrap_or(0),
        sha1: node["activeRevision"]["claimedDigests"]["sha1"]
            .as_str()
            .map(str::to_owned),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_traversal_and_bad_names() {
        for name in ["..", "../escape", "a/b", "a\\b", "\0", ""] {
            assert!(validate_name(name).is_err());
        }
        for path in [
            "/",
            "/my-files/../escape",
            "/my-files//foo",
            "/other",
            "/devices",
            "/devices/",
            "/devices/../escape",
            "/devicesx/PC",
        ] {
            assert!(validate_remote(path).is_err());
        }
        assert!(validate_remote("/my-files/Documentos pessoais").is_ok());
        assert!(validate_remote("/devices/Meu PC/Documentos").is_ok());
    }
    #[test]
    fn parses_real_sdk_shape() {
        let n = serde_json::json!({"uid":"u", "name":{"ok":true,"value":"f.txt"},"type":"file", "activeRevision":{"uid":"r","claimedSize":12}});
        let e = parse_entry(&n, "/my-files").unwrap();
        assert_eq!(e.revision, "u:r");
        assert_eq!(e.path, "/my-files/f.txt");
        let mut bad = n;
        bad["name"]["ok"] = Value::Bool(false);
        assert!(parse_entry(&bad, "/my-files").is_err());
    }
}
