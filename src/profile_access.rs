// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::{self, File, TryLockError};
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::args::Commands;

pub(crate) fn acquire(command: &Commands, data_dir: &Path) -> Result<Option<File>> {
    if matches!(command, Commands::Completions { .. }) {
        return Ok(None);
    }

    fs::create_dir_all(data_dir).context("Cannot create the data directory")?;
    let access = File::open(data_dir).context("Cannot open the data directory")?;
    // A database replacement changes the file's inode; the profile directory
    // stays stable so ordinary CLI opens cannot race the rename (#696).
    let lock = if matches!(
        command,
        Commands::Import { full: true, .. } | Commands::Init { force: true, .. }
    ) {
        access.try_lock()
    } else {
        access.try_lock_shared()
    };
    match lock {
        Ok(()) => Ok(Some(access)),
        Err(TryLockError::WouldBlock) => {
            bail!("Another Vauchi command is using this data directory. Close it and try again.")
        }
        Err(TryLockError::Error(error)) => Err(error).context("Cannot lock the data directory"),
    }
}
