// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Emergency Broadcast Commands
//!
//! Configure and send emergency alerts to trusted contacts.

use anyhow::{Result, bail};
use dialoguer::{Confirm, Input};

use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

/// Configure emergency broadcast (set trusted contacts + message).
pub fn configure(
    config: &CliConfig,
    contacts: Option<String>,
    message: Option<String>,
    include_location: bool,
) -> Result<()> {
    let mut wb = open_vauchi(config)?;

    // Flags bypass every prompt: the e2e certification harness drives this
    // command as a subprocess with no tty
    // (2026-07-24-duress-alert-e2e-coverage-gap).
    let (ids_input, message, include_location) = match contacts {
        Some(ids) => (
            ids,
            message.unwrap_or_else(|| vauchi_core::DEFAULT_EMERGENCY_MESSAGE.to_string()),
            include_location,
        ),
        None => {
            let ids: String = Input::new()
                .with_prompt("Trusted contact IDs (comma-separated, max 10)")
                .interact_text()?;
            let message: String = Input::new()
                .with_prompt("Alert message")
                .default(vauchi_core::DEFAULT_EMERGENCY_MESSAGE.to_string())
                .interact_text()?;
            let include_location = Confirm::new()
                .with_prompt("Include location in alert?")
                .default(false)
                .interact()?;
            (ids, message, include_location)
        }
    };

    let contact_ids: Vec<String> = ids_input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    if contact_ids.is_empty() {
        bail!("At least one contact ID is required");
    }

    wb.configure_emergency_broadcast(contact_ids, message, include_location)?;
    display::success("Emergency broadcast configured");

    Ok(())
}

/// Send emergency broadcast to all trusted contacts.
pub fn send(config: &CliConfig, yes: bool) -> Result<()> {
    let mut wb = open_vauchi(config)?;

    if wb.load_emergency_config()?.is_none() {
        bail!("No emergency broadcast configured. Run 'vauchi emergency configure' first.");
    }

    let confirmed = yes
        || Confirm::new()
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

/// List received safety alerts (emergency and duress).
///
/// Surfacing rides `surface_pending_safety_alerts` so Core owns the whole
/// contract (durable facts, at-least-once, ADR-056 blocked-contact silence) —
/// this command only renders the dispatched events.
pub fn alerts(config: &CliConfig) -> Result<()> {
    use std::sync::{Arc, Mutex};
    use vauchi_core::VauchiEvent;

    let wb = open_vauchi(config)?;

    let lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&lines);
    wb.events().on_event(move |event| {
        let rendered = match event {
            VauchiEvent::EmergencyAlertReceived {
                contact_id,
                message,
                timestamp,
                alert_nonce,
                ..
            } => format!(
                "ALERT kind=emergency contact={} nonce={} at={} message={}",
                contact_id,
                hex::encode(alert_nonce),
                timestamp,
                message
            ),
            VauchiEvent::DuressAlertReceived {
                contact_id,
                message,
                timestamp,
                alert_nonce,
                ..
            } => format!(
                "ALERT kind=duress contact={} nonce={} at={} message={}",
                contact_id,
                hex::encode(alert_nonce),
                timestamp,
                message
            ),
            _ => return,
        };
        sink.lock().expect("alert sink").push(rendered);
    });

    wb.surface_pending_safety_alerts()?;

    let lines = lines.lock().expect("alert sink");
    if lines.is_empty() {
        display::info("No pending alerts");
    } else {
        for line in lines.iter() {
            println!("{line}");
        }
    }

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
            if cfg.is_default_message() {
                println!("  Alert Message:      (default)");
            } else {
                println!("  Alert Message:      (custom)");
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
