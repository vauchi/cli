// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! GDPR Commands
//!
//! Privacy compliance operations: data export, identity deletion, consent management.

use std::path::Path;

use anyhow::{Result, bail};
use dialoguer::Input;
use vauchi_core::Vauchi;
use vauchi_core::api::{
    ConsentManager, ConsentType, DeletionManager, PreSignedPurgeRequest, PurgeSender, ShredError,
    ShredManager, ShredReport, ShredToken, ShredVerification,
};
use vauchi_core::network::{HttpTransportAdapter, RelayClient, RelayClientConfig};
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
        let encrypted = wb.export_personal_data_encrypted(pw)?;
        crate::config::write_restricted(output, &encrypted)?;
        display::success(&format!("Encrypted GDPR data export saved to {:?}", output));
    } else {
        let export = wb.export_personal_data()?;
        let json = serde_json::to_string_pretty(&export)?;
        display::warning(
            "Exporting without encryption. Consider using --encrypt to protect sensitive data.",
        );
        crate::config::write_restricted(output, &json)?;
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

const SECONDS_PER_DAY: u64 = 86_400;
const SECONDS_PER_HOUR: u64 = 3_600;

fn prompt(text: &str) -> Result<String> {
    Ok(Input::new().with_prompt(text).interact_text()?)
}

/// Whole days and remaining hours in a span of seconds.
fn days_and_hours(seconds: u64) -> (u64, u64) {
    (
        seconds / SECONDS_PER_DAY,
        (seconds % SECONDS_PER_DAY) / SECONDS_PER_HOUR,
    )
}

/// Schedules identity deletion with 7-day grace period.
pub fn schedule_deletion(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;
    let answer = prompt(
        "This will schedule your identity for deletion in 7 days. Type 'delete' to confirm",
    )?;
    let recovery_answer = if confirms_deletion(&answer) && trusts_anyone_for_recovery(&wb)? {
        display::info(
            "Your trusted contacts can help you move your contacts to a new identity later. \
             If you keep recovery, the relay holds the encrypted list of who you trust for \
             up to 365 days. If you erase everything, that list goes with the rest, and \
             recovery is no longer possible.",
        );
        Some(prompt(
            "Type 'keep' to keep recovery or 'erase' to erase everything",
        )?)
    } else {
        None
    };
    match schedule_deletion_answered(&wb, &answer, recovery_answer.as_deref())? {
        Some(days) => {
            display::warning(&format!(
                "Identity deletion scheduled. You have {} days to cancel.",
                days
            ));
            display::info("Run 'vauchi gdpr cancel-deletion' to cancel.");
        }
        None => display::info("Deletion cancelled."),
    }
    Ok(())
}

fn confirms_deletion(answer: &str) -> bool {
    answer.to_lowercase() == "delete"
}

/// Whether the identity has anyone to keep recovery possible with; only
/// then is there a choice to make when deleting.
fn trusts_anyone_for_recovery(wb: &Vauchi) -> Result<bool> {
    Ok(wb
        .list_contacts()?
        .iter()
        .any(|contact| contact.is_recovery_trusted()))
}

/// Schedules the deletion when `answer` is "delete" in any case; returns
/// the grace period in days, or `None` when the answer declined.
///
/// With anyone trusted for recovery, `recovery_answer` must be "keep" or
/// "erase"; with nobody trusted there is nothing to keep, so the deletion
/// erases everything (#711).
fn schedule_deletion_answered(
    wb: &Vauchi,
    answer: &str,
    recovery_answer: Option<&str>,
) -> Result<Option<u64>> {
    if !confirms_deletion(answer) {
        return Ok(None);
    }
    let erase_guardian_set = if trusts_anyone_for_recovery(wb)? {
        match recovery_answer.map(|a| a.trim().to_lowercase()).as_deref() {
            Some("keep") => false,
            Some("erase") => true,
            _ => bail!("Answer 'keep' to keep recovery or 'erase' to erase everything."),
        }
    } else {
        true
    };
    let manager = DeletionManager::new(wb.storage());
    manager.schedule_deletion_erasing_guardian_set(erase_guardian_set)?;
    Ok(match manager.deletion_state()? {
        DeletionState::Scheduled {
            scheduled_at,
            execute_at,
            ..
        } => Some(days_and_hours(execute_at - scheduled_at).0),
        _ => None,
    })
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
    // Countdown against persisted `execute_at` — injectable CLI clock.
    for line in deletion_status_lines(&state, crate::clock::unix_seconds()) {
        println!("{line}");
    }
    Ok(())
}

/// What `gdpr deletion-status` prints for `state` at `now`.
fn deletion_status_lines(state: &DeletionState, now: u64) -> Vec<String> {
    match state {
        DeletionState::None => vec![display::info_line("No deletion scheduled.")],
        DeletionState::Scheduled {
            scheduled_at,
            execute_at,
            ..
        } => {
            let (days, hours) = days_and_hours(execute_at.saturating_sub(now));
            vec![
                display::warning_line(&format!(
                    "Deletion scheduled at {} — {} days, {} hours remaining.",
                    scheduled_at, days, hours
                )),
                display::info_line("Run 'vauchi gdpr cancel-deletion' to cancel."),
            ]
        }
        DeletionState::Executed { executed_at } => vec![display::warning_line(&format!(
            "Identity was destroyed at {}.",
            executed_at
        ))],
        _ => vec![display::info_line("Unknown deletion state.")],
    }
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
    wb.grant_consent(consent_type)?;

    display::success(&format!("Consent granted for: {}", type_str));
    Ok(())
}

/// Revokes consent for a specific type.
pub fn revoke_consent(config: &CliConfig, type_str: &str) -> Result<()> {
    let wb = open_vauchi(config)?;
    let consent_type = parse_consent_type(type_str)?;
    wb.revoke_consent(consent_type)?;

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

/// Creates a connected RelayClient for shred operations, routing every
/// relay-bound action through the OHTTP path that core already wired
/// for this Vauchi instance (gateway key from memory, the validated
/// storage cache, or the bundled config). Without an OHTTP route the
/// transport stays fail-closed: purge and revocation deliveries error
/// instead of leaking onto a direct connection (RG-8).
fn create_shred_relay_client(
    wb: &Vauchi,
    relay_url: &str,
    identity_id: &str,
) -> Result<RelayClient<HttpTransportAdapter>> {
    let http_url = ws_to_http(relay_url);
    let transport = wb.build_relay_transport(&http_url, 10_000);
    // The URL travels inside `transport`; HttpTransportAdapter ignores the
    // TransportConfig it is handed, so the client config stays default.
    let adapter = HttpTransportAdapter::new(transport);
    let mut client = RelayClient::new(
        adapter,
        RelayClientConfig::default(),
        identity_id.to_string(),
    );
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

/// The shred token for a scheduled deletion whose grace period has ended
/// at `now`; an error naming why otherwise.
fn shred_token_after_grace(state: &DeletionState, now: u64) -> Result<ShredToken> {
    match state {
        DeletionState::Scheduled {
            scheduled_at,
            execute_at,
            ..
        } => {
            if now < *execute_at {
                let (days, hours) = days_and_hours(execute_at.saturating_sub(now));
                bail!(
                    "Grace period has not elapsed. {} days, {} hours remaining.",
                    days,
                    hours
                );
            }
            Ok(ShredToken::from_created_at(*scheduled_at))
        }
        DeletionState::None => {
            bail!("No deletion scheduled. Run 'vauchi gdpr schedule-deletion' first.")
        }
        DeletionState::Executed { .. } => bail!("Identity has already been destroyed."),
        _ => bail!("Unknown deletion state."),
    }
}

/// Executes a scheduled identity deletion after the grace period.
pub async fn execute_deletion(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;
    let state = DeletionManager::new(wb.storage()).deletion_state()?;
    // Grace-period gate against persisted `execute_at` — injectable CLI
    // clock so E2E can fast-forward past the grace period.
    let token = shred_token_after_grace(&state, crate::clock::unix_seconds())?;

    let answer = prompt(
        "This will permanently destroy all data and notify contacts. Type 'EXECUTE' to confirm",
    )?;
    match execute_deletion_answered(config, &wb, token, &answer)? {
        Some(lines) => print_lines(&lines),
        None => display::info("Deletion cancelled."),
    }
    Ok(())
}

/// Destroys the identity when `answer` is exactly "EXECUTE"; returns the
/// report to print, or `None` when the answer declined.
fn execute_deletion_answered(
    config: &CliConfig,
    wb: &Vauchi,
    token: ShredToken,
    answer: &str,
) -> Result<Option<Vec<String>>> {
    execute_deletion_purging_with(config, wb, token, answer, |erase_guardian_set| {
        wb.purge_relay_data(erase_guardian_set)?;
        Ok(())
    })
}

/// Hands the shred a purge that is signed when the shred sends it, with
/// the recovery choice made at scheduling. The pre-signed purge would be
/// stale: the relay accepts a purge only within 60 seconds of its
/// timestamp (#711).
struct PurgeSignedAtSend<F> {
    erase_guardian_set: bool,
    purge: F,
}

impl<F: FnMut(bool) -> Result<()>> PurgeSender for PurgeSignedAtSend<F> {
    fn send_purge(
        &mut self,
        _pre_signed: &PreSignedPurgeRequest,
        _now: u64,
    ) -> Result<bool, ShredError> {
        Ok((self.purge)(self.erase_guardian_set).is_ok())
    }
}

/// [`execute_deletion_answered`] with `purge` standing in for the relay
/// purge, so the recovery choice it receives can be observed.
fn execute_deletion_purging_with(
    config: &CliConfig,
    wb: &Vauchi,
    token: ShredToken,
    answer: &str,
    purge: impl FnMut(bool) -> Result<()>,
) -> Result<Option<Vec<String>>> {
    if answer != "EXECUTE" {
        return Ok(None);
    }
    // Read before the shred: executing it leaves the deletion state behind.
    let erase_guardian_set = DeletionManager::new(wb.storage()).scheduled_erases_guardian_set()?;
    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("No identity found"))?;
    let secure_storage = create_secure_storage(config)?;
    let identity_id = hex::encode(identity.signing_public_key());
    let shred_manager = ShredManager::new(
        wb.storage(),
        secure_storage.as_ref(),
        identity,
        &config.data_dir,
    );

    let mut purge = PurgeSignedAtSend {
        erase_guardian_set,
        purge,
    };
    let mut revocation_client = create_shred_relay_client(wb, &config.relay_url, &identity_id)?;

    let mut lines = vec![display::info_line("Destroying identity...")];
    let report = shred_manager
        .hard_shred(token, Some(&mut purge), Some(&mut revocation_client))
        .map_err(|e| anyhow::anyhow!("Shred failed: {}", e))?;
    if !report.relay_purge_sent {
        lines.push(display::warning_line(
            "The relay could not be reached to purge your data; the local deletion continued.",
        ));
    }
    lines.extend(shred_summary_lines(&report, &shred_manager.verify_shred()));
    lines.push(display::success_line("Identity destroyed. Goodbye."));
    Ok(Some(lines))
}

/// Emergency immediate deletion — no grace period.
pub async fn panic_shred(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;
    if wb.identity().is_none() {
        bail!("No identity found");
    }
    let answer =
        prompt("EMERGENCY: This will immediately destroy ALL data. Type 'PANIC' to confirm")?;
    match panic_shred_answered(config, &wb, &answer)? {
        Some(lines) => print_lines(&lines),
        None => display::info("Panic shred cancelled."),
    }
    Ok(())
}

/// Destroys everything at once when `answer` is exactly "PANIC"; returns
/// the report to print, or `None` when the answer declined.
fn panic_shred_answered(
    config: &CliConfig,
    wb: &Vauchi,
    answer: &str,
) -> Result<Option<Vec<String>>> {
    if answer != "PANIC" {
        return Ok(None);
    }
    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("No identity found"))?;
    let secure_storage = create_secure_storage(config)?;
    let shred_manager = ShredManager::new(
        wb.storage(),
        secure_storage.as_ref(),
        identity,
        &config.data_dir,
    );

    // Sends nothing: a wipe concerns this device, the identity lives on in
    // its other devices (vauchi/private#910).
    let mut lines = vec![display::warning_line("Executing emergency panic shred...")];

    let report = shred_manager
        .panic_shred(None, None)
        .map_err(|e| anyhow::anyhow!("Panic shred failed: {}", e))?;
    lines.extend(shred_summary_lines(&report, &shred_manager.verify_shred()));
    lines.push(display::success_line(
        "Panic shred complete. All data destroyed.",
    ));
    Ok(Some(lines))
}

fn print_lines(lines: &[String]) {
    for line in lines {
        println!("{line}");
    }
}

/// The shred report and its verification, as printed after a shred.
fn shred_summary_lines(report: &ShredReport, verification: &ShredVerification) -> Vec<String> {
    vec![
        String::new(),
        display::info_line("=== Shred Report ==="),
        format!("  Contacts notified:      {}", report.contacts_notified),
        format!("  Relay purge sent:       {}", report.relay_purge_sent),
        format!("  Devices notified:       {}", report.devices_notified),
        format!("  SMK destroyed:          {}", report.smk_destroyed),
        format!(
            "  Identity file destroyed:{}",
            report.identity_file_destroyed
        ),
        format!("  Key files destroyed:    {}", report.key_files_destroyed),
        format!("  SQLite destroyed:       {}", report.sqlite_destroyed),
        format!("  Pre-signed deleted:     {}", report.pre_signed_deleted),
        format!("  Data dir deleted:       {}", report.data_dir_deleted),
        String::new(),
        display::info_line("=== Shred Verification ==="),
        format!("  SMK absent:        {}", verification.smk_absent),
        format!("  Database absent:   {}", verification.database_absent),
        format!("  Data dir absent:   {}", verification.data_dir_absent),
        format!("  Pre-signed absent: {}", verification.pre_signed_absent),
        if verification.all_clear {
            display::success_line("  All clear — all data verified destroyed.")
        } else {
            display::warning_line("  WARNING: Some data may not have been fully destroyed.")
        },
    ]
}

fn parse_consent_type(s: &str) -> Result<ConsentType> {
    ConsentType::parse(s).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown consent type: '{}'. Valid types: data_processing, contact_sharing, recovery_vouching",
            s
        )
    })
}

// INLINE_TEST_REQUIRED: Binary crate without lib.rs - tests cannot be external
#[cfg(test)]
#[path = "gdpr_tests.rs"]
mod gdpr_tests;
