use crate::{
    cli::{validate_name, validate_remote, Cli},
    model::{ComputerRegistration, RemoteEntry, SyncPair},
    Result,
};
use serde_json::Value;
use std::collections::BTreeSet;

pub enum RegistrationTarget {
    Existing(RemoteEntry),
    Create(String),
}

/// Resolve by identity once bound. Names are only used for an initial creation
/// or for retrying its persisted intent after an uncertain network outcome.
pub fn registration_target(
    binding: Option<&ComputerRegistration>,
    devices: &[RemoteEntry],
    requested_name: &str,
    existing_uid: Option<&str>,
) -> Result<RegistrationTarget> {
    if let Some(uid) = binding.and_then(|r| r.device_uid.as_deref()) {
        return devices.iter().find(|d| d.uid == uid).cloned()
            .map(RegistrationTarget::Existing)
            .ok_or_else(|| "This PC’s registration is no longer in Drive. Check the trash or restore the computer in the web app; another will not be created automatically.".into());
    }
    if binding.is_none() {
        if let Some(uid) = existing_uid {
            return devices
                .iter()
                .find(|d| d.uid == uid && d.media_type == "Linux")
                .cloned()
                .map(RegistrationTarget::Existing)
                .ok_or_else(|| "Select an existing Linux computer in this account.".into());
        }
    }
    let name = binding.map_or(requested_name, |r| r.name.as_str()).trim();
    validate_name(name)?;
    if name.len() > 120 {
        return Err("Computer name is too long.".into());
    }
    let matches: Vec<_> = devices.iter().filter(|d| d.name == name).collect();
    match matches.as_slice() {
        [] => Ok(RegistrationTarget::Create(name.into())),
        [device] if device.media_type == "Linux" => {
            Ok(RegistrationTarget::Existing((*device).clone()))
        }
        [_] => {
            Err("That name already belongs to another computer. Choose a different name.".into())
        }
        _ => Err("Computers have the same name. Rename them in Proton Drive.".into()),
    }
}

pub fn parse_devices(nodes: &[Value]) -> Result<Vec<RemoteEntry>> {
    let mut names = BTreeSet::new();
    nodes
        .iter()
        .map(|node| {
            let name = node["name"]
                .as_str()
                .or_else(|| {
                    (node["name"]["ok"].as_bool() == Some(true))
                        .then(|| node["name"]["value"].as_str())
                        .flatten()
                })
                .ok_or("Couldn't decrypt a computer name.")?;
            validate_name(name)?;
            if !names.insert(name) {
                return Err(
                    "Computers have duplicate names. Rename them in Proton Drive before syncing."
                        .into(),
                );
            }
            let uid = node["uid"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or("Computer has no identifier.")?;
            let root = node["rootFolderUid"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or("Computer has no root folder.")?;
            Ok(RemoteEntry {
                name: name.into(),
                path: format!("/devices/{name}"),
                uid: uid.into(),
                revision: root.into(),
                directory: true,
                kind: "device".into(),
                media_type: node["type"].as_str().unwrap_or("Computer").into(),
                modified: node["lastSyncTime"].as_str().map(str::to_owned),
                ..Default::default()
            })
        })
        .collect()
}

impl Cli {
    pub async fn computers(&self) -> Result<Vec<RemoteEntry>> {
        let output = self
            .run(&["filesystem", "list", "/devices", "--json"], 120)
            .await?;
        let nodes: Vec<Value> = serde_json::from_str(&output).map_err(|e| {
            crate::i18n::message(
                "Invalid computers: {0}",
                &[crate::i18n::nested(e.to_string())],
            )
        })?;
        parse_devices(&nodes)
    }

    pub async fn computer_for_path(&self, path: &str) -> Result<Option<RemoteEntry>> {
        validate_remote(path)?;
        let Some(name) = path
            .strip_prefix("/devices/")
            .and_then(|p| p.split('/').next())
        else {
            return Ok(None);
        };
        self.computers()
            .await?
            .into_iter()
            .find(|d| d.name == name)
            .map(Some)
            .ok_or_else(|| {
                "Computer not found. Check whether it was renamed or removed in Drive.".into()
            })
    }

    pub async fn verify_computer_pair(&self, pair: &SyncPair) -> Result<()> {
        if let Some(device) = self.computer_for_path(&pair.remote_path).await? {
            if pair.device_uid.as_deref() != Some(device.uid.as_str()) {
                return Err("The destination computer changed. Reconfigure this pairing to verify its identity.".into());
            }
        }
        Ok(())
    }

    pub async fn register_computer(&self, name: &str) -> Result<RemoteEntry> {
        validate_name(name)?;
        if name.len() > 120 {
            return Err("Computer name is too long.".into());
        }
        let output = self.run(&["device", "ensure", name, "--json"], 180).await?;
        let node: Value = serde_json::from_str(&output).map_err(|e| {
            crate::i18n::message(
                "Invalid registration: {0}",
                &[crate::i18n::nested(e.to_string())],
            )
        })?;
        parse_devices(&[node])?
            .pop()
            .ok_or_else(|| "Registration did not return a computer.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_devices_and_rejects_ambiguous_or_unverified_names() {
        let device = serde_json::json!({"uid":"device-1","rootFolderUid":"volume~root","type":"Linux","name":{"ok":true,"value":"Meu PC"}});
        let entry = parse_devices(std::slice::from_ref(&device))
            .unwrap()
            .remove(0);
        assert_eq!(entry.path, "/devices/Meu PC");
        assert_eq!(entry.uid, "device-1");
        assert_eq!(entry.revision, "volume~root");
        assert!(parse_devices(&[device.clone(), device.clone()]).is_err());
        let mut bad = device;
        bad["name"]["ok"] = Value::Bool(false);
        assert!(parse_devices(&[bad]).is_err());
    }
}

#[cfg(test)]
mod registration_tests {
    use super::*;
    fn pc(uid: &str, name: &str) -> RemoteEntry {
        RemoteEntry {
            uid: uid.into(),
            name: name.into(),
            path: format!("/devices/{name}"),
            media_type: "Linux".into(),
            ..Default::default()
        }
    }
    #[test]
    fn registered_identity_survives_renames_and_repeated_requests() {
        let binding = ComputerRegistration {
            name: "Nome antigo".into(),
            device_uid: Some("pc-1".into()),
        };
        let devices = [pc("pc-1", "Nome novo"), pc("pc-2", "Outro PC")];
        let RegistrationTarget::Existing(found) =
            registration_target(Some(&binding), &devices, "Terceiro nome", Some("pc-2")).unwrap()
        else {
            panic!("must reuse identity")
        };
        assert_eq!(found.uid, "pc-1");
        assert_eq!(found.path, "/devices/Nome novo");
        assert!(registration_target(Some(&binding), &[], "Create another", None).is_err());
    }
    #[test]
    fn persisted_pending_intent_recovers_uncertain_creation_without_a_duplicate() {
        let binding = ComputerRegistration {
            name: "Original".into(),
            device_uid: None,
        };
        let RegistrationTarget::Create(name) =
            registration_target(Some(&binding), &[], "Changed", None).unwrap()
        else {
            panic!("expected original intent")
        };
        assert_eq!(name, "Original");
        let RegistrationTarget::Existing(found) = registration_target(
            Some(&binding),
            &[pc("created", "Original")],
            "Changed",
            None,
        )
        .unwrap() else {
            panic!("must find previous attempt")
        };
        assert_eq!(found.uid, "created");
    }
    #[test]
    fn legacy_registration_can_be_linked_explicitly_and_is_scoped_to_the_account() {
        let devices = [pc("existing", "Custom name")];
        assert!(matches!(
            registration_target(None, &devices, "host", Some("existing")).unwrap(),
            RegistrationTarget::Existing(_)
        ));
        assert!(registration_target(None, &devices, "host", Some("missing")).is_err());
        let mut config = crate::model::Config::default();
        config.computer_registrations.insert(
            "account-a".into(),
            ComputerRegistration {
                name: "Custom name".into(),
                device_uid: Some("existing".into()),
            },
        );
        let restored: crate::model::Config =
            serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert!(!restored.computer_registrations.contains_key("account-b"));
        assert_eq!(
            restored.computer_registrations["account-a"]
                .device_uid
                .as_deref(),
            Some("existing")
        );
    }
}
