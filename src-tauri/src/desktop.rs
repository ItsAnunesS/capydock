use std::{
    fs,
    io::{self, Write},
    path::Path,
};

// https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html
fn quote_exec(path: &Path) -> io::Result<String> {
    let path = path
        .to_str()
        .ok_or_else(|| io::Error::other("Executable path is not UTF-8"))?;
    if path.contains(['\n', '\r', '\0', '=']) {
        return Err(io::Error::other(
            "Executable path cannot be represented in a desktop entry",
        ));
    }
    let quoted = path
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('`', "\\`")
        .replace('$', "\\$");
    Ok(format!(
        "\"{}\"",
        quoted.replace('\\', "\\\\").replace('%', "%%")
    ))
}

/// Keep startup preferences while moving the old product's registration to CapyDock.
/// Existing CapyDock entries win, including entries explicitly disabled by the user.
pub fn migrate_autostart(config: &Path, executable: &Path) -> io::Result<()> {
    let directory = config.join("autostart");
    let legacy = directory.join("Proton Drive Desktop.desktop");
    let content = match fs::read_to_string(&legacy) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if !content
        .lines()
        .any(|line| line == "Name=Proton Drive Desktop")
        || !content
            .lines()
            .any(|line| line == "Comment=Proton Drive Desktop startup script")
    {
        return Ok(());
    }
    let current = directory.join("CapyDock.desktop");
    if !current.try_exists()? {
        let exec = quote_exec(executable)?;
        let updated = content
            .lines()
            .map(|line| {
                if line.starts_with("Exec=") {
                    format!("Exec={exec}")
                } else {
                    line.replace("Proton Drive Desktop", "CapyDock")
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let mut file = tempfile::NamedTempFile::new_in(&directory)?;
        file.write_all(updated.as_bytes())?;
        file.as_file().sync_all()?;
        match file.persist_noclobber(&current) {
            Ok(_) => (),
            Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => (),
            Err(error) => return Err(error.error),
        }
    }
    fs::remove_file(legacy)
}

/// AppImage managers commonly register capydock.desktop. GTK/Wayland matches
/// our stable application ID instead, so keep a hidden alias with the same icon
/// and executable. NoDisplay avoids adding a second application to the menu.
pub fn align_appimage_launcher(data: &Path, executable: &Path) -> io::Result<()> {
    let applications = data.join("applications");
    let content = match fs::read_to_string(applications.join("capydock.desktop")) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let exec = quote_exec(executable)?;
    let plain_exec = format!("Exec={}", executable.display());
    let quoted_exec = format!("Exec={exec}");
    if !content.lines().any(|line| line == "Name=CapyDock")
        || !content
            .lines()
            .any(|line| line == plain_exec || line == quoted_exec)
    {
        return Ok(());
    }
    let destination = applications.join("io.github.protondrive.desktop.desktop");
    let previous = match fs::read_to_string(&destination) {
        Ok(previous) => {
            if !previous
                .lines()
                .any(|line| matches!(line, "Name=CapyDock" | "Name=Proton Drive Desktop"))
            {
                return Ok(());
            }
            previous
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    let updated = content
        .lines()
        .filter(|line| !line.starts_with("NoDisplay="))
        .map(|line| {
            if line == "[Desktop Entry]" {
                "[Desktop Entry]\nNoDisplay=true"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    if previous != updated {
        let mut file = tempfile::NamedTempFile::new_in(applications)?;
        file.write_all(updated.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist(destination).map_err(|error| error.error)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEGACY: &str = "[Desktop Entry]\nName=Proton Drive Desktop\nComment=Proton Drive Desktop startup script\nExec=/old/proton-drive-desktop\nHidden=true\nX-GNOME-Autostart-enabled=false\n";

    #[test]
    fn migration_keeps_disabled_preferences_and_uses_the_current_appimage() {
        let config = tempfile::tempdir().unwrap();
        let directory = config.path().join("autostart");
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("Proton Drive Desktop.desktop"), LEGACY).unwrap();
        let executable = Path::new("/home/user/My Apps/CapyDock.AppImage");
        migrate_autostart(config.path(), executable).unwrap();
        let content = fs::read_to_string(directory.join("CapyDock.desktop")).unwrap();
        assert!(content.contains("Name=CapyDock\n"));
        assert!(content.contains("Exec=\"/home/user/My Apps/CapyDock.AppImage\"\n"));
        assert!(content.contains("Hidden=true\nX-GNOME-Autostart-enabled=false\n"));
        assert!(!directory.join("Proton Drive Desktop.desktop").exists());
        migrate_autostart(config.path(), executable).unwrap();
        assert_eq!(
            fs::read_to_string(directory.join("CapyDock.desktop")).unwrap(),
            content
        );
    }

    #[test]
    fn migration_preserves_an_existing_registration_and_does_not_enable_new_installs() {
        let config = tempfile::tempdir().unwrap();
        let executable = Path::new("/new/CapyDock.AppImage");
        migrate_autostart(config.path(), executable).unwrap();
        assert!(!config.path().join("autostart").exists());
        let directory = config.path().join("autostart");
        fs::create_dir(&directory).unwrap();
        fs::write(directory.join("Proton Drive Desktop.desktop"), LEGACY).unwrap();
        let current = "[Desktop Entry]\nName=CapyDock\nExec=/custom/capydock\nHidden=true\n";
        fs::write(directory.join("CapyDock.desktop"), current).unwrap();
        migrate_autostart(config.path(), executable).unwrap();
        assert_eq!(
            fs::read_to_string(directory.join("CapyDock.desktop")).unwrap(),
            current
        );
        assert!(!directory.join("Proton Drive Desktop.desktop").exists());
    }

    #[test]
    fn unrelated_legacy_entries_and_invalid_paths_are_preserved() {
        let config = tempfile::tempdir().unwrap();
        let directory = config.path().join("autostart");
        fs::create_dir(&directory).unwrap();
        let legacy = directory.join("Proton Drive Desktop.desktop");
        fs::write(&legacy, "[Desktop Entry]\nName=Other application\n").unwrap();
        migrate_autostart(config.path(), Path::new("/new/app")).unwrap();
        assert!(legacy.exists());
        fs::write(&legacy, LEGACY).unwrap();
        assert!(migrate_autostart(config.path(), Path::new("/bad\npath")).is_err());
        assert_eq!(fs::read_to_string(legacy).unwrap(), LEGACY);
        assert!(!directory.join("CapyDock.desktop").exists());
    }

    #[test]
    fn executable_paths_escape_desktop_entry_and_exec_syntax() {
        assert_eq!(
            quote_exec(Path::new(r#"/apps/a\b$`"%"#)).unwrap(),
            r#""/apps/a\\\\b\\$\\`\\"%%""#
        );
    }

    #[test]
    fn appimage_alias_keeps_the_registered_icon_and_does_not_duplicate_the_menu() {
        let data = tempfile::tempdir().unwrap();
        let applications = data.path().join("applications");
        fs::create_dir(&applications).unwrap();
        let original = "[Desktop Entry]\nName=CapyDock\nExec=/apps/capydock.appimage\nIcon=/apps/icons/capydock.png\nNoDisplay=false\n";
        fs::write(applications.join("capydock.desktop"), original).unwrap();
        fs::write(
            applications.join("io.github.protondrive.desktop.desktop"),
            "[Desktop Entry]\nName=CapyDock\nExec=/old/proton-drive-desktop\n",
        )
        .unwrap();
        let executable = Path::new("/apps/capydock.appimage");
        align_appimage_launcher(data.path(), executable).unwrap();
        let alias =
            fs::read_to_string(applications.join("io.github.protondrive.desktop.desktop")).unwrap();
        assert!(alias.contains("Icon=/apps/icons/capydock.png\n"));
        assert!(alias.contains("NoDisplay=true\n"));
        assert!(!alias.contains("NoDisplay=false"));
        assert!(alias.contains("Exec=/apps/capydock.appimage\n"));
        assert_eq!(
            fs::read_to_string(applications.join("capydock.desktop")).unwrap(),
            original
        );
        align_appimage_launcher(data.path(), executable).unwrap();
        assert_eq!(
            fs::read_to_string(applications.join("io.github.protondrive.desktop.desktop")).unwrap(),
            alias
        );
    }

    #[test]
    fn unrelated_launchers_and_other_appimage_installations_are_untouched() {
        let data = tempfile::tempdir().unwrap();
        let executable = Path::new("/apps/capydock.appimage");
        align_appimage_launcher(data.path(), executable).unwrap();
        assert!(!data.path().join("applications").exists());
        let applications = data.path().join("applications");
        fs::create_dir(&applications).unwrap();
        fs::write(
            applications.join("capydock.desktop"),
            "[Desktop Entry]\nName=CapyDock\nExec=/other/capydock.appimage\n",
        )
        .unwrap();
        align_appimage_launcher(data.path(), executable).unwrap();
        assert!(!applications
            .join("io.github.protondrive.desktop.desktop")
            .exists());
        fs::write(
            applications.join("capydock.desktop"),
            "[Desktop Entry]\nName=CapyDock\nExec=/apps/capydock.appimage\n",
        )
        .unwrap();
        fs::write(
            applications.join("io.github.protondrive.desktop.desktop"),
            "Name=Another app\n",
        )
        .unwrap();
        align_appimage_launcher(data.path(), executable).unwrap();
        assert_eq!(
            fs::read_to_string(applications.join("io.github.protondrive.desktop.desktop")).unwrap(),
            "Name=Another app\n"
        );
    }
}
