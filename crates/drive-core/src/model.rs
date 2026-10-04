use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    Pt,
    Es,
    #[default]
    #[serde(other)]
    En,
}

fn deserialize_locale<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Locale, D::Error> {
    Ok(Option::<Locale>::deserialize(deserializer)?.unwrap_or_default())
}

impl Locale {
    pub fn library_names(self) -> [&'static str; 3] {
        match self {
            Self::Pt => ["Arquivos", "Fotos", "Álbuns"],
            Self::En => ["Files", "Photos", "Albums"],
            Self::Es => ["Archivos", "Fotos", "Álbumes"],
        }
    }

    pub fn tray_labels(self) -> (&'static str, &'static str) {
        match self {
            Self::Pt => ("Abrir CapyDock", "Sair"),
            Self::En => ("Open CapyDock", "Quit"),
            Self::Es => ("Abrir CapyDock", "Salir"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    Bidirectional,
    Upload,
    Download,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPair {
    pub id: String,
    pub name: String,
    pub local_path: String,
    pub remote_path: String,
    #[serde(default)]
    pub device_uid: Option<String>,
    pub mode: SyncMode,
    pub interval_minutes: u64,
    pub enabled: bool,
    #[serde(default)]
    pub last_run: Option<u64>,
    #[serde(default)]
    pub propagate_deletions: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    #[serde(deserialize_with = "deserialize_locale")]
    pub locale: Locale,
    pub pairs: Vec<SyncPair>,
    pub auto_update: bool,
    pub paused: bool,
    pub close_to_tray: bool,
    /// One binding per Proton account on this installation. A missing UID is a
    /// persisted creation intent, so a retry cannot create a PC under a new name.
    pub computer_registrations: BTreeMap<String, ComputerRegistration>,
    pub last_update_check: Option<u64>,
    pub events: Vec<Activity>,
    pub account_id: Option<String>,
    pub account_email: Option<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            locale: Locale::default(),
            pairs: vec![],
            auto_update: true,
            paused: false,
            close_to_tray: false,
            computer_registrations: BTreeMap::new(),
            last_update_check: None,
            events: vec![],
            account_id: None,
            account_email: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerRegistration {
    pub name: String,
    pub device_uid: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub id: String,
    pub timestamp: u64,
    pub kind: String,
    pub message: String,
    pub pair_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fingerprint {
    pub hash: String,
    pub size: u64,
    pub directory: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Baseline {
    pub local: Fingerprint,
    pub remote: String,
}
pub type Snapshot = BTreeMap<String, Baseline>;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteEntry {
    pub name: String,
    pub path: String,
    pub directory: bool,
    pub revision: String,
    pub size: u64,
    pub sha1: Option<String>,
    #[serde(default)]
    pub uid: String,
    #[serde(default)]
    pub media_type: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub native_document: bool,
    #[serde(default)]
    pub photo_count: u64,
    #[serde(default)]
    pub modified: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub uploaded: u64,
    pub downloaded: u64,
    pub unchanged: u64,
    pub deleted: u64,
    pub online_only: u64,
    pub conflicts: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Upload,
    Download,
    Unchanged,
    Conflict,
    Ignore,
}

/// Missing files are never interpreted as permission to delete the other copy.
pub fn decide(
    local: Option<&Fingerprint>,
    remote: Option<&RemoteEntry>,
    baseline: Option<&Baseline>,
    mode: SyncMode,
) -> Action {
    use Action::*;
    match (local, remote) {
        (None, None) => Ignore,
        (Some(l), Some(r)) if l.directory != r.directory => Conflict,
        (Some(l), Some(_)) if l.directory => Unchanged,
        (Some(l), Some(r)) => {
            let Some(b) = baseline else {
                return Conflict;
            };
            let lc = l != &b.local;
            let rc = r.revision != b.remote;
            match (lc, rc, mode) {
                (false, false, _) => Unchanged,
                (true, true, _) => Conflict,
                (true, false, SyncMode::Download) | (false, true, SyncMode::Upload) => Conflict,
                (true, false, _) => Upload,
                (false, true, _) => Download,
            }
        }
        (Some(_), None) => {
            if baseline.is_some() {
                Conflict
            } else if mode == SyncMode::Download {
                Ignore
            } else {
                Upload
            }
        }
        (None, Some(_)) => {
            if baseline.is_some() {
                Conflict
            } else if mode == SyncMode::Upload {
                Ignore
            } else {
                Download
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_defaults_for_existing_settings_and_round_trips_without_changing_them() {
        let original = r#"{"paused":true,"autoUpdate":false,"accountId":"account","events":[]}"#;
        let mut config: Config = serde_json::from_str(original).unwrap();
        assert_eq!(config.locale, Locale::En);
        assert!(!config.close_to_tray);
        config.close_to_tray = true;
        for locale in [Locale::Pt, Locale::En, Locale::Es] {
            config.locale = locale;
            let dir = tempfile::tempdir().unwrap();
            let file = dir.path().join("settings.json");
            crate::atomic_json(&file, &config).unwrap();
            let restored: Config = serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
            assert_eq!(restored.locale, locale);
            assert!(restored.paused);
            assert!(restored.close_to_tray);
            assert!(!restored.auto_update);
            assert_eq!(restored.account_id.as_deref(), Some("account"));
        }
        assert_eq!(
            serde_json::from_str::<Locale>(r#""fr""#).unwrap(),
            Locale::En
        );
    }

    #[test]
    fn english_is_the_default_and_fallback_without_resetting_other_preferences() {
        assert_eq!(Config::default().locale, Locale::En);
        assert_eq!(
            Locale::default().library_names(),
            ["Files", "Photos", "Albums"]
        );
        assert_eq!(Locale::default().tray_labels(), ("Open CapyDock", "Quit"));
        for json in [
            r#"{"paused":true,"autoUpdate":false,"closeToTray":true}"#,
            r#"{"locale":"unsupported","paused":true,"autoUpdate":false,"closeToTray":true}"#,
            r#"{"locale":null,"paused":true,"autoUpdate":false,"closeToTray":true}"#,
        ] {
            let config: Config = serde_json::from_str(json).unwrap();
            assert_eq!(config.locale, Locale::En);
            assert!(config.paused);
            assert!(config.close_to_tray);
            assert!(!config.auto_update);
        }
    }
    fn local(hash: &str) -> Fingerprint {
        Fingerprint {
            hash: hash.into(),
            size: 10,
            directory: false,
        }
    }
    fn remote(rev: &str) -> RemoteEntry {
        RemoteEntry {
            name: "a.txt".into(),
            path: "/my-files/a.txt".into(),
            directory: false,
            revision: rev.into(),
            size: 10,
            sha1: None,
            ..Default::default()
        }
    }
    #[test]
    fn sync_decisions_preserve_data() {
        let l = local("old");
        let r = remote("old");
        let b = Baseline {
            local: l.clone(),
            remote: r.revision.clone(),
        };
        let changed = local("new");
        let revised = remote("new");
        let cases = [
            (
                Some(&l),
                Some(&r),
                Some(&b),
                SyncMode::Bidirectional,
                Action::Unchanged,
            ),
            (
                Some(&changed),
                Some(&r),
                Some(&b),
                SyncMode::Bidirectional,
                Action::Upload,
            ),
            (
                Some(&l),
                Some(&revised),
                Some(&b),
                SyncMode::Bidirectional,
                Action::Download,
            ),
            (
                Some(&changed),
                Some(&revised),
                Some(&b),
                SyncMode::Bidirectional,
                Action::Conflict,
            ),
            (
                Some(&l),
                Some(&r),
                None,
                SyncMode::Bidirectional,
                Action::Conflict,
            ),
            (
                None,
                Some(&r),
                Some(&b),
                SyncMode::Bidirectional,
                Action::Conflict,
            ),
            (
                Some(&l),
                None,
                Some(&b),
                SyncMode::Bidirectional,
                Action::Conflict,
            ),
            (Some(&l), None, None, SyncMode::Upload, Action::Upload),
            (None, Some(&r), None, SyncMode::Download, Action::Download),
            (None, Some(&r), None, SyncMode::Upload, Action::Ignore),
            (
                Some(&changed),
                Some(&r),
                Some(&b),
                SyncMode::Download,
                Action::Conflict,
            ),
            (
                Some(&l),
                Some(&revised),
                Some(&b),
                SyncMode::Upload,
                Action::Conflict,
            ),
        ];
        for (l, r, b, m, expected) in cases {
            assert_eq!(decide(l, r, b, m), expected);
        }
    }
}
