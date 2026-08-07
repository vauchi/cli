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

/// Exports an identity backup.
pub fn export(config: &CliConfig, output: &Path, password: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("No identity found"))?;

    let backup = identity.export_backup(password)?;

    fs::write(output, backup.as_bytes())?;

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
    fs::write(output, backup_hex.as_bytes())?;

    display::success(&format!("Full backup saved to {:?}", output));
    display::warning(
        "This file contains your identity, contacts, and labels. Keep it and the password safe.",
    );

    Ok(())
}

/// Imports a full backup (identity + contacts + own card + labels).
pub fn import_full(config: &CliConfig, input: &Path, password: &str, yes: bool) -> Result<()> {
    if !confirm_overwrite(config, yes)? {
        return Ok(());
    }

    let backup_hex = fs::read_to_string(input)?;

    // Validate and decrypt BEFORE touching local state: a typo'd path or
    // wrong password must leave the existing install fully intact. Core
    // rejects full restores onto an initialized instance
    // (`AlreadyInitialized`), so the reset happens only after the backup
    // is proven readable.
    let backup_bytes =
        hex::decode(backup_hex.trim()).map_err(|e| anyhow::anyhow!("Invalid backup file: {e}"))?;
    vauchi_core::backup::import_full_backup(&backup_bytes, password)
        .map_err(|e| anyhow::anyhow!("Backup validation failed (wrong password?): {e}"))?;

    crate::commands::common::reset_local_state(config)?;

    fs::create_dir_all(&config.data_dir)?;

    let wb_config = VauchiConfig::with_storage_path(config.storage_path())
        .with_relay_url(&config.relay_url)
        .with_storage_key(config.storage_key()?);

    let mut wb = Vauchi::new(wb_config)?;
    // Core persists the restored identity (and contacts, card, labels) into
    // its encrypted storage — no frontend-side identity file needed.
    wb.import_full_backup(&backup_hex, password)?;

    let name = wb
        .identity()
        .map(|id| id.display_name().to_string())
        .ok_or_else(|| anyhow::anyhow!("Identity missing after restore"))?;

    let public_id = wb.public_id()?;

    display::success(&format!("Full backup restored: {}", name));
    println!();
    println!("  Public ID: {}", public_id);
    println!("  Data dir:  {:?}", config.data_dir);
    println!();
    display::info("Identity, contacts, own card, and labels have been restored.");

    Ok(())
}

/// Guards a restore onto an initialized workspace.
///
/// Fails closed on probe errors and returns false when the user cancels.
/// `yes` skips the prompt for scripted/E2E use (the probe still runs).
/// Never touches local state: callers decide whether a reset is needed
/// (full restores, after validating the backup) or not (identity-only
/// restores overwrite the identity row in place).
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
