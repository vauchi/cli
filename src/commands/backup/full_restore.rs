// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::{self, File};
use std::io;
use std::path::Path;

use anyhow::{Context, Result, bail};
use vauchi_core::{Vauchi, VauchiConfig};

use crate::config::CliConfig;
use crate::display;

pub(super) struct RestoredIdentity {
    pub name: String,
    pub public_id: String,
}

pub(super) fn check_target(config: &CliConfig) -> Result<()> {
    for path in [config.storage_path(), config.identity_path()] {
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_file() => {}
            Ok(_) => bail!("Cannot restore: {} must be a regular file", path.display()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("Cannot inspect the restore destination"),
        }
    }
    Ok(())
}

pub(super) fn restore(
    config: &CliConfig,
    backup_hex: &str,
    password: &str,
    publish: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> Result<RestoredIdentity> {
    check_target(config)?;
    let mut staging_builder = tempfile::Builder::new();
    staging_builder.prefix(".restore-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        staging_builder.permissions(fs::Permissions::from_mode(0o700));
    }
    let staging = staging_builder
        .tempdir_in(&config.data_dir)
        .context("Cannot prepare the restore directory")?;
    let staged_path = staging.path().join("vauchi.db");
    let wb_config = VauchiConfig::with_storage_path(&staged_path)
        .with_relay_url(&config.relay_url)
        .with_storage_key(config.storage_key()?);
    let mut replacement = Vauchi::new(wb_config)?;
    replacement
        .import_full_backup(backup_hex, password)
        .map_err(|error| anyhow::anyhow!("Backup validation failed (wrong password?): {error}"))?;
    let restored = RestoredIdentity {
        name: replacement
            .identity()
            .context("Identity missing after restore")?
            .display_name()
            .to_string(),
        public_id: replacement.public_id()?.to_string(),
    };
    replacement.storage().wal_checkpoint()?;
    drop(replacement);
    check_no_sidecars(&staged_path)?;
    restrict_permissions(&staged_path)?;
    File::open(&staged_path)?.sync_all()?;

    let destination = config.storage_path();
    check_no_sidecars(&destination)?;
    publish(&staged_path, &destination).context("Cannot publish the restored database")?;

    // Publication is the commit point. A later cleanup/flush error must
    // report the committed restore rather than claim it was rolled back.
    match fs::remove_file(config.identity_path()) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => display::warning(&format!(
            "Restore committed, but the legacy identity file could not be removed: {error}"
        )),
    }
    if let Err(error) = File::open(&config.data_dir).and_then(|dir| dir.sync_all()) {
        display::warning(&format!(
            "Restore committed, but flushing the data directory failed: {error}"
        ));
    }
    Ok(restored)
}

fn check_no_sidecars(database: &Path) -> Result<()> {
    for extension in ["db-wal", "db-shm"] {
        match fs::symlink_metadata(database.with_extension(extension)) {
            Ok(_) => bail!(
                "The database is not ready for replacement. Close other Vauchi processes and try again."
            ),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("Cannot inspect the database journals"),
        }
    }
    Ok(())
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) -> Result<()> {
    bail!("Full restore requires a platform with private file permissions")
}

#[cfg(all(test, unix))]
#[path = "../../../tests/commands/full_restore.rs"]
mod tests;
