pub mod catalog;
pub mod cli;
pub mod computers;
pub mod i18n;
pub mod model;
pub mod operations;
pub mod sync;
pub mod update;

pub type Result<T> = std::result::Result<T, String>;

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn atomic_json(path: &std::path::Path, value: &impl serde::Serialize) -> Result<()> {
    use std::io::Write;
    let parent = path.parent().ok_or("Caminho inválido")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
pub mod library;
pub mod local_root;
pub mod photos;
pub mod watch;
