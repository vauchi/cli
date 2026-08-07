// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Init Command
//!
//! Creates a new Vauchi identity.

use std::fs;

use anyhow::{Result, bail};
use vauchi_core::{Vauchi, VauchiConfig};

use crate::commands::common::{identity_exists, reset_local_state};
use crate::config::CliConfig;
use crate::display;

/// Creates a new identity.
pub fn run(name: &str, force: bool, config: &CliConfig, locale: &str) -> Result<()> {
    if identity_exists(config)? && !force {
        bail!(
            "Vauchi is already initialized in {:?}. Use --force to overwrite or --data-dir for a different location.",
            config.data_dir
        );
    }

    fs::create_dir_all(&config.data_dir)?;

    // When forcing, remove ALL old state (core DB + WAL sidecars + the
    // legacy identity file) so Vauchi::new() starts fresh and no stale
    // identity can be resurrected via the open_vauchi fallback.
    if force {
        reset_local_state(config)?;
    }

    let wb_config = VauchiConfig::with_storage_path(config.storage_path())
        .with_relay_url(&config.relay_url)
        .with_storage_key(config.storage_key()?);

    let mut wb = Vauchi::new(wb_config)?;
    // Core persists the new identity into its encrypted storage.
    wb.create_identity(name)?;

    if let Err(e) = wb.initialize_demo_contact() {
        // Non-fatal: demo contact is a nice-to-have, not blocking
        eprintln!(
            "{}",
            display::tf(
                "cli.cmd.init.demo_skipped",
                locale,
                &[("error", &e.to_string())]
            )
        );
    }

    let public_id = wb.public_id()?;

    display::success(&format!("Identity created: {}", name));
    println!();
    println!("  Public ID: {}", public_id);
    println!("  Data dir:  {:?}", config.data_dir);
    println!();
    display::info("Add contact info with: vauchi card add <type> <label> <value>");

    Ok(())
}
