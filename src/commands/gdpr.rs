// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! GDPR Commands
//!
//! Privacy compliance operations: data export, identity deletion, consent management.

use std::fs;
use std::path::Path;

use anyhow::{Result, bail};
use dialoguer::Input;
use vauchi_core::api::{
    ConsentManager, ConsentType, DeletionManager, ShredManager, ShredReport, ShredToken,
    ShredVerification, export_all_data, export_encrypted,
};
use vauchi_core::network::{
    HttpTransport, HttpTransportAdapter, HttpTransportConfig, ProxyConfig, RelayClient,
    RelayClientConfig, TransportConfig,
};
use vauchi_core::storage::DeletionState;
use vauchi_core::storage::secure::SecureStorage;

use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

/// Exports all user data as GDPR-compliant JSON.
///
/// If `password` is provided, uses core's encrypted export envelope
/// (Argon2id + HKDF domain separation + XChaCha20-Poly1305).
pub fn export_data(config: &CliConfig, output: &Path, password: Option<&str>) -> Result<()> {
    let wb = open_vauchi(config)?;

    if let Some(pw) = password {
        let encrypted = export_encrypted(wb.storage(), pw)?;
        fs::write(output, &encrypted)?;
        display::success(&format!("Encrypted GDPR data export saved to {:?}", output));
    } else {
        let export = export_all_data(wb.storage())?;
        let json = serde_json::to_string_pretty(&export)?;
        display::warning(
            "Exporting without encryption. Consider using --encrypt to protect sensitive data.",
        );
        fs::write(output, &json)?;
        display::success(&format!("GDPR data export saved to {:?}", output));

        display::info(&format!(
            "Export version: {}, contacts: {}, exported at: {}",
            export.version,
            export.contacts.len(),
            export.exported_at
        ));
    }

    Ok(())
}

/// Schedules identity deletion with 7-day grace period.
pub fn schedule_deletion(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;

    let confirm: String = Input::new()
        .with_prompt(
            "This will schedule your identity for deletion in 7 days. Type 'delete' to confirm",
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
            "Identity deletion scheduled. You have {} days to cancel.",
            days
        ));
        display::info("Run 'vauchi gdpr cancel-deletion' to cancel.");
    }

    Ok(())
}

/// Cancels a scheduled identity deletion.
pub fn cancel_deletion(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;
    let manager = DeletionManager::new(wb.storage());
    manager.cancel_deletion()?;

    display::success("Identity deletion cancelled.");
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
            display::warning(&format!("Identity was destroyed at {}.", executed_at));
        }
        _ => {
            display::info("Unknown deletion state.");
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

/// Creates a SecureStorage instance matching the platform config pattern.
#[allow(unused_variables)]
fn create_secure_storage(config: &CliConfig) -> Result<Box<dyn SecureStorage>> {
    #[cfg(feature = "secure-storage")]
    {
        Ok(Box::new(
            vauchi_core::storage::secure::PlatformKeyring::new("vauchi-cli"),
        ))
    }

    #[cfg(not(feature = "secure-storage"))]
    {
        let fallback_key = crate::config::load_or_generate_fallback_key(&config.data_dir)?;
        let key_dir = config.data_dir.join("keys");
        Ok(Box::new(vauchi_core::storage::secure::FileKeyStorage::new(
            key_dir,
            fallback_key,
        )))
    }
}

/// Creates a connected RelayClient for shred operations using HTTP transport.
fn create_relay_client(
    relay_url: &str,
    identity_id: &str,
) -> Result<RelayClient<HttpTransportAdapter>> {
    let http_url = ws_to_http(relay_url);
    let transport = HttpTransport::new(HttpTransportConfig {
        relay_url: http_url.clone(),
        timeout_ms: 10_000,
        proxy: ProxyConfig::None,
        allow_direct: true,
    });
    let adapter = HttpTransportAdapter::new(transport);
    let transport_config = TransportConfig {
        server_url: http_url,
        ..TransportConfig::default()
    };
    let config = RelayClientConfig {
        transport: transport_config,
        ..RelayClientConfig::default()
    };
    let mut client = RelayClient::new(adapter, config, identity_id.to_string());
    client
        .connect()
        .map_err(|e| anyhow::anyhow!("Failed to connect to relay: {}", e))?;
    Ok(client)
}

/// Convert wss:// to https:// and ws:// to http://.
fn ws_to_http(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("wss://") {
        format!("https://{rest}")
    } else if let Some(rest) = url.strip_prefix("ws://") {
        format!("http://{rest}")
    } else {
        url.to_string()
    }
}

/// Executes a scheduled identity deletion after the grace period.
pub async fn execute_deletion(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;
    let identity = config.import_local_identity()?;

    // Verify deletion is scheduled and grace period elapsed
    let manager = DeletionManager::new(wb.storage());
    let state = manager.deletion_state()?;
    let token = match state {
        DeletionState::Scheduled {
            scheduled_at,
            execute_at,
        } => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if now < execute_at {
                let remaining = execute_at.saturating_sub(now);
                let days = remaining / 86400;
                let hours = (remaining % 86400) / 3600;
                bail!(
                    "Grace period has not elapsed. {} days, {} hours remaining.",
                    days,
                    hours
                );
            }
            ShredToken::from_created_at(scheduled_at)
        }
        DeletionState::None => {
            bail!("No deletion scheduled. Run 'vauchi gdpr schedule-deletion' first.")
        }
        DeletionState::Executed { .. } => bail!("Identity has already been destroyed."),
        _ => bail!("Unknown deletion state."),
    };

    // Confirmation prompt
    let confirm: String = Input::new()
        .with_prompt(
            "This will permanently destroy all data and notify contacts. Type 'EXECUTE' to confirm",
        )
        .interact_text()?;

    if confirm != "EXECUTE" {
        display::info("Deletion cancelled.");
        return Ok(());
    }

    let secure_storage = create_secure_storage(config)?;
    let identity_id = hex::encode(identity.signing_public_key());
    let shred_manager = ShredManager::new(
        wb.storage(),
        secure_storage.as_ref(),
        &identity,
        &config.data_dir,
    );

    // Create two separate relay clients (borrow rules: PurgeSender + RevocationSender)
    let mut purge_client = create_relay_client(&config.relay_url, &identity_id)?;
    let mut revocation_client = create_relay_client(&config.relay_url, &identity_id)?;

    display::info("Destroying identity...");

    let report = shred_manager
        .hard_shred(token, Some(&mut purge_client), Some(&mut revocation_client))
        .map_err(|e| anyhow::anyhow!("Shred failed: {}", e))?;

    display_shred_report(&report);
    let verification = shred_manager.verify_shred();
    display_shred_verification(&verification);

    display::success("Identity destroyed. Goodbye.");
    Ok(())
}

/// Emergency immediate deletion — no grace period.
pub async fn panic_shred(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;
    let identity = config.import_local_identity()?;

    // Confirmation prompt
    let confirm: String = Input::new()
        .with_prompt("EMERGENCY: This will immediately destroy ALL data. Type 'PANIC' to confirm")
        .interact_text()?;

    if confirm != "PANIC" {
        display::info("Panic shred cancelled.");
        return Ok(());
    }

    let secure_storage = create_secure_storage(config)?;
    let identity_id = hex::encode(identity.signing_public_key());
    let shred_manager = ShredManager::new(
        wb.storage(),
        secure_storage.as_ref(),
        &identity,
        &config.data_dir,
    );

    // Best-effort relay connections — failure doesn't block shred
    let mut purge_client = create_relay_client(&config.relay_url, &identity_id).ok();
    let mut revocation_client = create_relay_client(&config.relay_url, &identity_id).ok();

    if purge_client.is_none() || revocation_client.is_none() {
        display::warning("Could not connect to relay. Revocations will be best-effort.");
    }

    display::warning("Executing emergency panic shred...");

    let report = shred_manager
        .panic_shred(
            purge_client
                .as_mut()
                .map(|c| c as &mut dyn vauchi_core::api::PurgeSender),
            revocation_client
                .as_mut()
                .map(|c| c as &mut dyn vauchi_core::api::RevocationSender),
        )
        .map_err(|e| anyhow::anyhow!("Panic shred failed: {}", e))?;

    display_shred_report(&report);
    let verification = shred_manager.verify_shred();
    display_shred_verification(&verification);

    display::success("Panic shred complete. All data destroyed.");
    Ok(())
}

/// Displays a shred report summary.
fn display_shred_report(report: &ShredReport) {
    println!();
    display::info("=== Shred Report ===");
    println!("  Contacts notified:      {}", report.contacts_notified);
    println!("  Relay purge sent:       {}", report.relay_purge_sent);
    println!("  Devices notified:       {}", report.devices_notified);
    println!("  SMK destroyed:          {}", report.smk_destroyed);
    println!(
        "  Identity file destroyed:{}",
        report.identity_file_destroyed
    );
    println!("  Key files destroyed:    {}", report.key_files_destroyed);
    println!("  SQLite destroyed:       {}", report.sqlite_destroyed);
    println!("  Pre-signed deleted:     {}", report.pre_signed_deleted);
    println!("  Data dir deleted:       {}", report.data_dir_deleted);
}

/// Displays shred verification results.
fn display_shred_verification(verification: &ShredVerification) {
    println!();
    display::info("=== Shred Verification ===");
    println!("  SMK absent:        {}", verification.smk_absent);
    println!("  Database absent:   {}", verification.database_absent);
    println!("  Data dir absent:   {}", verification.data_dir_absent);
    println!("  Pre-signed absent: {}", verification.pre_signed_absent);
    if verification.all_clear {
        display::success("  All clear — all data verified destroyed.");
    } else {
        display::warning("  WARNING: Some data may not have been fully destroyed.");
    }
}

fn parse_consent_type(s: &str) -> Result<ConsentType> {
    ConsentType::parse(s).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown consent type: '{}'. Valid types: data_processing, contact_sharing, recovery_vouching",
            s
        )
    })
}
