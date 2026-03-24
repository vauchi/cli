// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tor Privacy Mode Commands
//!
//! Configure and manage Tor preferences.
//!
//! **WARNING**: Tor is NOT wired to the sync path. These commands save
//! preferences to storage but do NOT route traffic through Tor.
//! See problem record `2026-03-24-tor-ip-hiding-strategy`.

use anyhow::{Result, bail};
use vauchi_core::Storage;

use crate::config::CliConfig;
use crate::display;

/// Opens storage from the CLI config.
fn open_storage(config: &CliConfig) -> Result<Storage> {
    if !config.is_initialized() {
        bail!("Vauchi not initialized. Run 'vauchi init <name>' first.");
    }
    let key = config.storage_key()?;
    let storage = Storage::open(config.storage_path(), key)?;
    Ok(storage)
}

/// Enable Tor mode.
pub fn enable(config: &CliConfig) -> Result<()> {
    let storage = open_storage(config)?;
    let mut tor_config = storage.load_or_create_tor_config()?;

    if tor_config.enabled {
        display::info("Tor mode is already enabled");
        return Ok(());
    }

    tor_config.enabled = true;
    storage.save_tor_config(&tor_config)?;

    display::success("Tor preference saved (enabled)");
    display::warning(
        "Note: Tor is not yet wired to connections — traffic still uses direct WebSocket",
    );
    if tor_config.prefer_onion {
        display::info(".onion addresses will be preferred when available");
    }

    Ok(())
}

/// Disable Tor mode.
pub fn disable(config: &CliConfig) -> Result<()> {
    let storage = open_storage(config)?;
    let mut tor_config = storage.load_or_create_tor_config()?;

    if !tor_config.enabled {
        display::info("Tor mode is already disabled");
        return Ok(());
    }

    tor_config.enabled = false;
    storage.save_tor_config(&tor_config)?;

    display::success("Tor preference saved (disabled)");
    Ok(())
}

/// Show Tor status and configuration summary.
pub fn status(config: &CliConfig) -> Result<()> {
    let storage = open_storage(config)?;
    let tor_config = storage.load_or_create_tor_config()?;

    println!();
    println!(
        "  Tor Preference:    {}",
        if tor_config.enabled {
            "ENABLED (preference only — not wired to connections)"
        } else {
            "DISABLED"
        }
    );
    println!(
        "  Prefer .onion:     {}",
        if tor_config.prefer_onion { "yes" } else { "no" }
    );
    println!("  Circuit rotation:  {}s", tor_config.circuit_rotation_secs);
    println!(
        "  Bridges:           {}",
        if tor_config.has_bridges() {
            format!("{} configured", tor_config.bridges.len())
        } else {
            "none".to_string()
        }
    );
    if tor_config.enabled {
        println!();
        display::warning("Tor transport is not yet wired to the sync path.");
        display::warning("Traffic currently uses direct WebSocket connections.");
    }
    println!();

    Ok(())
}

/// Request a new Tor circuit (force circuit rotation).
pub fn new_circuit(config: &CliConfig) -> Result<()> {
    let storage = open_storage(config)?;
    let tor_config = storage.load_or_create_tor_config()?;

    if !tor_config.enabled {
        display::warning("Tor mode is not enabled");
        display::info("Enable it with: vauchi tor enable");
        return Ok(());
    }

    // Tor transport is not wired to the sync path.
    // Circuit rotation has no effect until Tor is integrated.
    display::warning("Tor transport is not yet wired to connections");
    display::info("Circuit rotation preference saved, but has no effect currently");
    Ok(())
}

/// Add bridge addresses.
pub fn bridges_add(config: &CliConfig, addr: &str) -> Result<()> {
    let storage = open_storage(config)?;
    let mut tor_config = storage.load_or_create_tor_config()?;

    if tor_config.bridges.contains(&addr.to_string()) {
        display::info("Bridge already configured");
        return Ok(());
    }

    tor_config.bridges.push(addr.to_string());
    storage.save_tor_config(&tor_config)?;

    display::success(&format!(
        "Bridge added (total: {})",
        tor_config.bridges.len()
    ));
    Ok(())
}

/// List configured bridge addresses.
pub fn bridges_list(config: &CliConfig) -> Result<()> {
    let storage = open_storage(config)?;
    let tor_config = storage.load_or_create_tor_config()?;

    if tor_config.bridges.is_empty() {
        display::info("No bridges configured");
        return Ok(());
    }

    println!();
    println!("  Configured bridges:");
    for (i, bridge) in tor_config.bridges.iter().enumerate() {
        println!("    {}. {}", i + 1, bridge);
    }
    println!();

    Ok(())
}

/// Clear all configured bridge addresses.
pub fn bridges_clear(config: &CliConfig) -> Result<()> {
    let storage = open_storage(config)?;
    let mut tor_config = storage.load_or_create_tor_config()?;

    if tor_config.bridges.is_empty() {
        display::info("No bridges to clear");
        return Ok(());
    }

    let count = tor_config.bridges.len();
    tor_config.bridges.clear();
    storage.save_tor_config(&tor_config)?;

    display::success(&format!("Cleared {} bridge(s)", count));
    Ok(())
}
