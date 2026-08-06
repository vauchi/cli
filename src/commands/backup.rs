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
pub fn import(config: &CliConfig, input: &Path, password: &str) -> Result<()> {
    if !confirm_overwrite(config)? {
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
pub fn import_full(config: &CliConfig, input: &Path, password: &str) -> Result<()> {
    if !confirm_overwrite(config)? {
        return Ok(());
    }

    let backup_hex = fs::read_to_string(input)?;

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
        .unwrap_or_default();

    let public_id = wb.public_id().ok();

    display::success(&format!("Full backup restored: {}", name));
    if let Some(public_id) = public_id {
        println!();
        println!("  Public ID: {}", public_id);
        println!("  Data dir:  {:?}", config.data_dir);
        println!();
    }
    display::info("Identity, contacts, own card, and labels have been restored.");

    Ok(())
}

/// Guards a restore onto an initialized workspace.
///
/// Returns false when the user cancels the overwrite prompt. On confirmation
/// the existing core storage and the legacy identity file are removed so the
/// restore lands on a fresh instance — core rejects imports onto an instance
/// that already holds an identity (`AlreadyInitialized`).
fn confirm_overwrite(config: &CliConfig) -> Result<bool> {
    if !identity_exists(config) {
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

    let storage_path = config.storage_path();
    if storage_path.exists() {
        fs::remove_file(&storage_path)?;
    }
    let identity_path = config.identity_path();
    if identity_path.exists() {
        fs::remove_file(&identity_path)?;
    }

    Ok(true)
}
