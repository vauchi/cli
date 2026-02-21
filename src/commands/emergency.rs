// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Emergency Broadcast Commands
//!
//! Configure and send emergency alerts to trusted contacts.

use anyhow::{bail, Result};
use dialoguer::{Confirm, Input};

use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

/// Configure emergency broadcast (set trusted contacts + message).
pub fn configure(config: &CliConfig) -> Result<()> {
    let mut wb = open_vauchi(config)?;

    // Get contact IDs (comma-separated)
    let ids_input: String = Input::new()
        .with_prompt("Trusted contact IDs (comma-separated, max 10)")
        .interact_text()?;

    let contact_ids: Vec<String> = ids_input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if contact_ids.is_empty() {
        bail!("At least one contact ID is required");
    }

    let message: String = Input::new()
        .with_prompt("Alert message")
        .default("I may be in danger. Please check on me.".to_string())
        .interact_text()?;

    let include_location = Confirm::new()
        .with_prompt("Include location in alert?")
        .default(false)
        .interact()?;

    wb.configure_emergency_broadcast(contact_ids, message, include_location)?;
    display::success("Emergency broadcast configured");

    Ok(())
}

/// Send emergency broadcast to all trusted contacts.
pub fn send(config: &CliConfig) -> Result<()> {
    let mut wb = open_vauchi(config)?;

    // Check config exists
    if wb.load_emergency_config()?.is_none() {
        bail!("No emergency broadcast configured. Run 'vauchi emergency configure' first.");
    }

    let confirmed = Confirm::new()
        .with_prompt("Send emergency alert to all trusted contacts?")
        .default(false)
        .interact()?;

    if !confirmed {
        display::info("Cancelled");
        return Ok(());
    }

    let result = wb.send_emergency_broadcast()?;
    display::success(&format!(
        "Emergency broadcast sent: {}/{} contacts reached",
        result.sent, result.total
    ));

    Ok(())
}

/// Show emergency broadcast status.
pub fn status(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;

    let config_opt = wb.load_emergency_config()?;

    println!();
    match config_opt {
        Some(cfg) => {
            println!("  Emergency Broadcast: CONFIGURED");
            println!(
                "  Trusted Contacts:   {} contact(s)",
                cfg.trusted_contact_ids.len()
            );
            if cfg.message != "I may be in danger. Please check on me." {
                println!("  Alert Message:      (custom)");
            } else {
                println!("  Alert Message:      (default)");
            }
            println!(
                "  Include Location:   {}",
                if cfg.include_location { "Yes" } else { "No" }
            );
        }
        None => {
            println!("  Emergency Broadcast: NOT CONFIGURED");
        }
    }
    println!();

    Ok(())
}

/// Disable emergency broadcast.
pub fn disable(config: &CliConfig) -> Result<()> {
    let mut wb = open_vauchi(config)?;

    if wb.load_emergency_config()?.is_none() {
        display::info("Emergency broadcast is not configured");
        return Ok(());
    }

    wb.delete_emergency_config()?;
    display::success("Emergency broadcast disabled");

    Ok(())
}
