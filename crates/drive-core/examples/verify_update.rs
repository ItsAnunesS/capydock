/// Downloads the current official release into a temporary directory and exercises
/// the same checksum, executable validation and atomic install used by the desktop.
#[tokio::main]
async fn main() -> Result<(), String> {
    let folder = tempfile::tempdir().map_err(|e| e.to_string())?;
    let release = drive_core::update::latest().await?;
    let destination = folder.path().join("proton-drive");
    drive_core::update::install(&release, &destination).await?;
    let cli = drive_core::cli::Cli {
        binary: destination,
    };
    println!(
        "{}\nSHA-512 and atomic installation verified.",
        cli.run(&["version"], 30).await?
    );
    Ok(())
}
