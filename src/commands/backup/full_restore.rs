// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io;
use std::path::Path;

use anyhow::{Context, Result};

use crate::commands::storage_replacement;
use crate::config::CliConfig;

pub(super) struct RestoredIdentity {
    pub name: String,
    pub public_id: String,
}

pub(super) fn check_target(config: &CliConfig) -> Result<()> {
    storage_replacement::check_target(config)
}

pub(super) fn restore(
    config: &CliConfig,
    backup_hex: &str,
    password: &str,
    publish: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> Result<RestoredIdentity> {
    storage_replacement::replace(
        config,
        ".restore-",
        |replacement| {
            replacement
                .import_full_backup(backup_hex, password)
                .map_err(|error| {
                    anyhow::anyhow!("Backup validation failed (wrong password?): {error}")
                })?;
            Ok(RestoredIdentity {
                name: replacement
                    .identity()
                    .context("Identity missing after restore")?
                    .display_name()
                    .to_string(),
                public_id: replacement.public_id()?.to_string(),
            })
        },
        publish,
    )
}

#[cfg(all(test, unix))]
#[path = "../../../tests/commands/full_restore.rs"]
mod tests;
