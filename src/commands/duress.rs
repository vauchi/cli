// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Duress PIN Commands
//!
//! Set up and manage duress PIN for plausible deniability.

use anyhow::{bail, Result};
use dialoguer::Password;
use vauchi_core::network::MockTransport;
use vauchi_core::{Vauchi, VauchiConfig};

use crate::config::CliConfig;
use crate::display;

/// Opens Vauchi from the config and loads the identity.
fn open_vauchi(config: &CliConfig) -> Result<Vauchi<MockTransport>> {
    if !config.is_initialized() {
        bail!("Vauchi not initialized. Run 'vauchi init <name>' first.");
    }

    let wb_config = VauchiConfig::with_storage_path(config.storage_path())
        .with_relay_url(&config.relay_url)
        .with_storage_key(config.storage_key()?);

    let mut wb = Vauchi::new(wb_config)?;

    let identity = config.import_local_identity()?;
    wb.set_identity(identity)?;

    Ok(wb)
}

/// Set up duress PIN.
pub fn setup(config: &CliConfig) -> Result<()> {
    let mut wb = open_vauchi(config)?;

    // Check if app password is set first
    if !wb.is_password_enabled()? {
        display::info("App password not set. Setting it up first...");
        let password = Password::new()
            .with_prompt("Enter new app password")
            .with_confirmation("Confirm app password", "Passwords do not match")
            .interact()?;
        wb.setup_app_password(&password)?;
        display::success("App password set");
    }

    // Now set duress PIN
    let duress = Password::new()
        .with_prompt("Enter duress PIN")
        .with_confirmation("Confirm duress PIN", "PINs do not match")
        .interact()?;

    wb.setup_duress_password(&duress)?;
    display::success("Duress PIN configured");
    display::info("When entered, contacts will be replaced with decoy data");

    Ok(())
}

/// Show duress status.
pub fn status(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;

    let password_enabled = wb.is_password_enabled()?;
    let duress_enabled = wb.is_duress_enabled()?;

    println!();
    println!(
        "  App Password:  {}",
        if password_enabled {
            "ENABLED"
        } else {
            "NOT SET"
        }
    );
    println!(
        "  Duress PIN:    {}",
        if duress_enabled { "ENABLED" } else { "NOT SET" }
    );

    if duress_enabled {
        if let Ok(Some(settings)) = wb.load_duress_settings() {
            println!(
                "  Alert Contacts: {}",
                if settings.alert_contact_ids.is_empty() {
                    "none configured".to_string()
                } else {
                    format!("{} contact(s)", settings.alert_contact_ids.len())
                }
            );
            if !settings.alert_message.is_empty() {
                println!("  Alert Message:  (custom)");
            }
        }
    }
    println!();

    Ok(())
}

/// Disable duress PIN.
pub fn disable(config: &CliConfig) -> Result<()> {
    let mut wb = open_vauchi(config)?;

    if !wb.is_duress_enabled()? {
        display::info("Duress PIN is not enabled");
        return Ok(());
    }

    wb.disable_duress()?;
    display::success("Duress PIN disabled");

    Ok(())
}

/// Test authentication (shows Normal/Duress result).
pub fn test(config: &CliConfig, pin: &str) -> Result<()> {
    let mut wb = open_vauchi(config)?;

    if !wb.is_password_enabled()? {
        bail!("No app password set. Run 'vauchi duress setup' first.");
    }

    let result = wb.authenticate(pin)?;
    match result {
        vauchi_core::AuthMode::Normal => display::success("Authentication result: Normal"),
        vauchi_core::AuthMode::Duress => display::warning("Authentication result: DURESS"),
        vauchi_core::AuthMode::Unauthenticated => {
            display::warning("Authentication result: Invalid")
        }
    }

    Ok(())
}
