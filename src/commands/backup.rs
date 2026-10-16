// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Backup Commands
//!
//! Export and import backups (identity-only or full).

use std::fs;
use std::path::Path;

use anyhow::Result;
use dialoguer::Input;
use vauchi_core::{Vauchi, VauchiConfig};

use crate::commands::common::{identity_exists, open_vauchi};
use crate::config::CliConfig;
use crate::display;

mod full_restore;

/// Exports an identity backup.
pub fn export(config: &CliConfig, output: &Path, password: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("No identity found"))?;

    let backup = identity.export_backup(password)?;

    crate::config::write_restricted(output, backup.as_bytes())?;

    display::success(&format!("Backup saved to {:?}", output));
    display::warning("Keep this file and password safe. You'll need both to restore.");

    Ok(())
}

/// Imports an identity from backup.
///
/// Identity-only restore overwrites the identity row in place — contacts,
/// labels, and the own card are preserved, so no local state is reset.
/// A wrong password fails inside core before anything is written.
pub fn import(config: &CliConfig, input: &Path, password: &str, yes: bool) -> Result<()> {
    if !confirm_overwrite(config, yes)? {
        return Ok(());
    }

    let backup_data = fs::read(input)?;

    fs::create_dir_all(&config.data_dir)?;

    let wb_config = VauchiConfig::with_storage_path(config.storage_path())
        .with_relay_url(&config.relay_url)
        .with_storage_key(config.storage_key()?);

    let mut wb = Vauchi::new(wb_config)?;
    // Core owns the restore: import_backup validates, persists the identity
    // into encrypted storage, and creates the default contact card.
    wb.import_backup(&hex::encode(&backup_data), password)?;

    let name = wb
        .identity()
        .map(|id| id.display_name().to_string())
        .ok_or_else(|| anyhow::anyhow!("Identity missing after restore"))?;
    let public_id = wb.public_id()?;

    display::success(&format!("Identity restored: {}", name));
    println!();
    println!("  Public ID: {}", public_id);
    println!("  Data dir:  {:?}", config.data_dir);
    println!();
    display::info("Your contacts and card will need to sync from the relay.");

    Ok(())
}

/// Exports a full backup (identity + contacts + own card + labels).
pub fn export_full(config: &CliConfig, output: &Path, password: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let backup_hex = wb.export_full_backup(password)?;
    crate::config::write_restricted(output, backup_hex.as_bytes())?;

    display::success(&format!("Full backup saved to {:?}", output));
    display::warning(
        "This file contains your identity, contacts, and labels. Keep it and the password safe.",
    );

    Ok(())
}

/// Imports a full backup (identity + contacts + own card + labels).
pub fn import_full(config: &CliConfig, input: &Path, password: &str, yes: bool) -> Result<()> {
    full_restore::check_target(config)?;
    if !confirm_overwrite(config, yes)? {
        return Ok(());
    }

    let backup_hex = fs::read_to_string(input)?;

    let restored = full_restore::restore(config, &backup_hex, password, |from, to| {
        fs::rename(from, to)
    })?;

    display::success(&format!("Full backup restored: {}", restored.name));
    println!();
    println!("  Public ID: {}", restored.public_id);
    println!("  Data dir:  {:?}", config.data_dir);
    println!();
    display::info("Identity, contacts, own card, and labels have been restored.");

    Ok(())
}

/// Guards a restore onto an initialized workspace.
///
/// Fails closed on probe errors and returns false when the user cancels.
/// `yes` skips the prompt for scripted/E2E use (the probe still runs).
/// Full restores stage a replacement database; identity-only restores
/// update the identity row in existing storage.
fn confirm_overwrite(config: &CliConfig, yes: bool) -> Result<bool> {
    if !identity_exists(config)? {
        return Ok(true);
    }

    if yes {
        return Ok(true);
    }

    display::warning("Vauchi is already initialized.");

    let confirm: String = Input::new()
        .with_prompt("This will overwrite existing data. Type 'yes' to continue")
        .interact_text()?;

    if confirm.to_lowercase() != "yes" {
        display::info("Import cancelled.");
        return Ok(false);
    }

    Ok(true)
}
