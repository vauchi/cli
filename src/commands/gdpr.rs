// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! GDPR Commands
//!
//! Privacy compliance operations: data export, account deletion, consent management.

use std::fs;
use std::path::Path;

use anyhow::{bail, Result};
use dialoguer::Input;
use vauchi_core::api::{export_all_data, ConsentManager, ConsentType, DeletionManager};
use vauchi_core::network::MockTransport;
use vauchi_core::storage::DeletionState;
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

/// Exports all user data as GDPR-compliant JSON.
pub fn export_data(config: &CliConfig, output: &Path) -> Result<()> {
    let wb = open_vauchi(config)?;
    let export = export_all_data(wb.storage())?;

    let json = serde_json::to_string_pretty(&export)?;
    fs::write(output, &json)?;

    display::success(&format!("GDPR data export saved to {:?}", output));
    display::info(&format!(
        "Export version: {}, contacts: {}, exported at: {}",
        export.version,
        export.contacts.len(),
        export.exported_at
    ));

    Ok(())
}

/// Schedules account deletion with 7-day grace period.
pub fn schedule_deletion(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;

    let confirm: String = Input::new()
        .with_prompt(
            "This will schedule your account for deletion in 7 days. Type 'delete' to confirm",
        )
        .interact_text()?;

    if confirm.to_lowercase() != "delete" {
        display::info("Deletion cancelled.");
        return Ok(());
    }

    let manager = DeletionManager::new(wb.storage());
    manager.schedule_deletion()?;

    let state = manager.deletion_state()?;
    if let DeletionState::Scheduled {
        scheduled_at,
        execute_at,
    } = state
    {
        let days = (execute_at - scheduled_at) / 86400;
        display::warning(&format!(
            "Account deletion scheduled. You have {} days to cancel.",
            days
        ));
        display::info("Run 'vauchi gdpr cancel-deletion' to cancel.");
    }

    Ok(())
}

/// Cancels a scheduled account deletion.
pub fn cancel_deletion(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;
    let manager = DeletionManager::new(wb.storage());
    manager.cancel_deletion()?;

    display::success("Account deletion cancelled.");
    Ok(())
}

/// Shows current deletion state.
pub fn deletion_status(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;
    let manager = DeletionManager::new(wb.storage());
    let state = manager.deletion_state()?;

    match state {
        DeletionState::None => {
            display::info("No deletion scheduled.");
        }
        DeletionState::Scheduled {
            scheduled_at,
            execute_at,
        } => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let remaining = execute_at.saturating_sub(now);
            let days = remaining / 86400;
            let hours = (remaining % 86400) / 3600;

            display::warning(&format!(
                "Deletion scheduled at {} — {} days, {} hours remaining.",
                scheduled_at, days, hours
            ));
            display::info("Run 'vauchi gdpr cancel-deletion' to cancel.");
        }
        DeletionState::Executed { executed_at } => {
            display::warning(&format!("Account was deleted at {}.", executed_at));
        }
    }

    Ok(())
}

/// Shows consent status for all consent types.
pub fn consent_status(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;
    let manager = ConsentManager::new(wb.storage());
    let records = manager.export_consent_log_with_version()?;

    if records.is_empty() {
        display::info("No consent records found.");
        return Ok(());
    }

    println!(
        "{:<20} {:<10} {:<15} {:<15}",
        "Type", "Granted", "Timestamp", "Policy Version"
    );
    println!("{}", "-".repeat(60));

    for record in &records {
        let granted = if record.granted { "Yes" } else { "No" };
        let pv = record.policy_version.as_deref().unwrap_or("-");
        println!(
            "{:<20} {:<10} {:<15} {:<15}",
            format!("{:?}", record.consent_type),
            granted,
            record.timestamp,
            pv
        );
    }

    Ok(())
}

/// Grants consent for a specific type.
pub fn grant_consent(config: &CliConfig, type_str: &str) -> Result<()> {
    let wb = open_vauchi(config)?;
    let consent_type = parse_consent_type(type_str)?;
    let manager = ConsentManager::new(wb.storage());
    manager.grant(consent_type)?;

    display::success(&format!("Consent granted for: {}", type_str));
    Ok(())
}

/// Revokes consent for a specific type.
pub fn revoke_consent(config: &CliConfig, type_str: &str) -> Result<()> {
    let wb = open_vauchi(config)?;
    let consent_type = parse_consent_type(type_str)?;
    let manager = ConsentManager::new(wb.storage());
    manager.revoke(consent_type)?;

    display::success(&format!("Consent revoked for: {}", type_str));
    Ok(())
}

fn parse_consent_type(s: &str) -> Result<ConsentType> {
    ConsentType::parse(s).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown consent type: '{}'. Valid types: data_processing, contact_sharing, analytics, recovery_vouching",
            s
        )
    })
}
