// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Init Command
//!
//! Creates a new Vauchi identity.

use std::fs;

use anyhow::{Result, bail};
use vauchi_core::{Vauchi, VauchiConfig};

use crate::config::CliConfig;
use crate::display;

/// Creates a new identity.
pub fn run(name: &str, force: bool, config: &CliConfig) -> Result<()> {
    // Check if already initialized
    if config.is_initialized() && !force {
        bail!(
            "Vauchi is already initialized in {:?}. Use --force to overwrite or --data-dir for a different location.",
            config.data_dir
        );
    }

    // Create data directory
    fs::create_dir_all(&config.data_dir)?;

    // When forcing, remove old storage so Vauchi::new() starts fresh
    if force {
        let storage_path = config.storage_path();
        if storage_path.exists() {
            fs::remove_file(&storage_path)?;
        }
    }

    // Initialize Vauchi with persistent storage key
    let wb_config = VauchiConfig::with_storage_path(config.storage_path())
        .with_relay_url(&config.relay_url)
        .with_storage_key(config.storage_key()?);

    let mut wb = Vauchi::new(wb_config)?;
    wb.create_identity(name)?;

    // Initialize demo contact for new users with no contacts
    if let Err(e) = wb.initialize_demo_contact() {
        // Non-fatal: demo contact is a nice-to-have, not blocking
        eprintln!("Note: demo contact setup skipped: {}", e);
    }

    // Save identity to file for persistence
    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("Identity not found after creation"))?;
    config.save_local_identity(identity)?;

    // Get identity info
    let public_id = wb.public_id()?;

    display::success(&format!("Identity created: {}", name));
    println!();
    println!("  Public ID: {}", public_id);
    println!("  Data dir:  {:?}", config.data_dir);
    println!();
    display::info("Add contact info with: vauchi card add <type> <label> <value>");

    Ok(())
}
