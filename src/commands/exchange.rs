// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exchange Command
//!
//! Generate and complete contact exchanges.

use std::fs;

use anyhow::{Result, bail};
use vauchi_core::contact_card::ContactCard;
use vauchi_core::exchange::{
    ExchangeEvent, ExchangeQR, ExchangeSession, ExchangeState, ManualConfirmationVerifier,
};
use vauchi_core::sync::{ContactSyncData, DeviceSyncOrchestrator, SyncItem};
use vauchi_core::types::{AhaMomentTracker, AhaMomentType};
use vauchi_core::{Contact, Identity, Vauchi};

use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

/// Records a new contact addition for inter-device sync.
fn record_contact_added(wb: &Vauchi, contact: &Contact) -> Result<()> {
    // Try to load device registry - if none exists, skip (single device)
    let registry = match wb.storage().load_device_registry()? {
        Some(r) if r.device_count() > 1 => r,
        _ => return Ok(()), // No other devices to sync to
    };

    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("No identity found"))?;

    // Load orchestrator with existing state (not new(), which would overwrite previous items)
    let mut orchestrator =
        DeviceSyncOrchestrator::load(wb.storage(), identity.create_device_info(), registry)
            .unwrap_or_else(|_| {
                DeviceSyncOrchestrator::new(
                    wb.storage(),
                    identity.create_device_info(),
                    identity.initial_device_registry(),
                )
            });

    let contact_data = ContactSyncData::from_contact(contact);
    let item = SyncItem::ContactAdded {
        contact_data,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    };

    if let Err(e) = orchestrator.record_local_change(item) {
        display::warning(&format!("Could not record sync item: {:?}", e));
    }

    Ok(())
}

/// Starts a contact exchange by generating a QR code.
///
/// Uses ExchangeSession state machine with ManualConfirmationVerifier
/// since CLI doesn't have audio hardware for proximity verification.
pub fn start(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;

    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("No identity found"))?;

    let our_card = wb
        .storage()
        .load_own_card()?
        .unwrap_or_else(|| ContactCard::new(identity.display_name()));

    let verifier = ManualConfirmationVerifier::new();

    let backup_password = config.backup_password()?;
    let backup = identity.export_backup(&backup_password)?;
    let identity_owned = Identity::import_backup(&backup, &backup_password)?;

    let mut session = ExchangeSession::new_qr(identity_owned, our_card, verifier);

    session
        .apply(ExchangeEvent::StartQR)
        .map_err(|e| anyhow::anyhow!("Failed to generate QR: {:?}", e))?;

    let (qr_data, qr_image) = match session.qr() {
        Some(qr) => (qr.to_data_string(), qr.to_qr_image_string()),
        None => bail!("QR code not generated"),
    };

    display::info("Share this with another Vauchi user:");
    println!();
    println!("{}", qr_image);
    println!();
    println!("Or share this data string:");
    println!("  {}", qr_data);
    println!();

    display::info("After they complete the exchange, run 'vauchi sync' to receive their info.");

    Ok(())
}

/// Completes a contact exchange with received data.
///
/// Uses the mutual QR exchange flow: both sides generate a QR and scan each
/// other's QR code. The mutual scan serves as proximity verification.
/// Flow: StartQR -> ProcessQR -> TheyScannedOurQR -> PerformKeyAgreement -> CompleteExchange.
///
/// After creating the contact, queues our initial card for delivery
/// and runs a sync to send it immediately.
pub fn complete(config: &CliConfig, data: &str) -> Result<()> {
    let mut wb = open_vauchi(config)?;

    let qr = ExchangeQR::from_data_string(data)?;

    if qr.is_expired() {
        bail!("This exchange QR code has expired. Ask them to generate a new one.");
    }

    let their_exchange_key = *qr.exchange_key();
    let their_public_id = hex::encode(qr.public_key());

    if wb.get_contact(&their_public_id)?.is_some() {
        display::warning("You already have this contact.");
        return Ok(());
    }

    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("No identity found"))?;

    let our_card = wb
        .storage()
        .load_own_card()?
        .unwrap_or_else(|| ContactCard::new(identity.display_name()));

    let verifier = ManualConfirmationVerifier::new();

    let backup_password = config.backup_password()?;
    let backup = identity.export_backup(&backup_password)?;
    let identity_owned = Identity::import_backup(&backup, &backup_password)?;

    let mut session = ExchangeSession::new_qr(identity_owned, our_card, verifier);

    session
        .apply(ExchangeEvent::StartQR)
        .map_err(|e| anyhow::anyhow!("Failed to start QR: {:?}", e))?;

    if let Some(our_qr) = session.qr() {
        let qr_data = our_qr.to_data_string();
        let qr_image = our_qr.to_qr_image_string();
        display::info("Your QR code (share with the other user):");
        println!();
        println!("{}", qr_image);
        println!();
        println!("Or share this data string:");
        println!("  {}", qr_data);
        println!();
    }

    session
        .apply(ExchangeEvent::ProcessQR(qr))
        .map_err(|e| anyhow::anyhow!("Failed to process QR: {:?}", e))?;

    session
        .apply(ExchangeEvent::TheyScannedOurQR)
        .map_err(|e| anyhow::anyhow!("Failed to confirm they scanned our QR: {:?}", e))?;

    session
        .apply(ExchangeEvent::PerformKeyAgreement)
        .map_err(|e| anyhow::anyhow!("Key agreement failed: {:?}", e))?;

    let shared_key = match session.state() {
        ExchangeState::AwaitingCardExchange { shared_key, .. } => shared_key.clone(),
        _ => bail!("Session not in expected state after key agreement"),
    };

    let their_name = session
        .their_display_name()
        .filter(|n| !n.is_empty())
        .unwrap_or("New Contact")
        .to_string();
    let their_card = ContactCard::new(&their_name);
    session
        .apply(ExchangeEvent::CompleteExchange(their_card))
        .map_err(|e| anyhow::anyhow!("Card exchange failed: {:?}", e))?;

    let contact = match session.state() {
        ExchangeState::Complete { contact } => *contact.clone(),
        _ => bail!("Session not in Complete state"),
    };

    let contact_id = contact.id().to_string();
    let contact_clone = contact.clone();

    wb.add_contact(contact)?;

    // Aha moment: first contact added
    let mut tracker = load_aha_tracker(config);
    if let Some(moment) =
        tracker.try_trigger_with_context(AhaMomentType::FirstContactAdded, their_name.to_string())
    {
        display::display_aha_moment(&moment);
    }
    save_aha_tracker(config, &tracker);

    if let Err(e) = record_contact_added(&wb, &contact_clone) {
        display::warning(&format!("Could not record for device sync: {}", e));
    }

    wb.create_ratchet_as_initiator(&contact_id, &shared_key, their_exchange_key)?;

    // Queue our card for delivery and sync immediately.
    // The initial card establishes the responder's receive chain so
    // both parties can send updates.
    match wb.queue_initial_card_for_contact(&contact_id) {
        Ok(()) => {
            if let Err(e) = wb.connect() {
                display::warning(&format!("Could not connect to relay: {e}"));
            } else if let Err(e) = wb.sync() {
                display::warning(&format!("Could not sync: {e}"));
            } else {
                display::info("Sent initial card to enable bidirectional messaging");
            }
            wb.disconnect();
        }
        Err(e) => {
            display::warning(&format!("Could not prepare initial card: {e}"));
            display::info("Run 'vauchi sync' to send your card later.");
        }
    }

    // Note: C1 post-exchange delay is in-memory only (Instant). It cannot
    // survive the CLI's per-command Vauchi lifecycle. C1 is effective in
    // long-lived instances (mobile apps, TUI) but not in CLI. No call to
    // set_post_exchange_delay() here — it would be misleading.

    println!();
    display::success(&format!(
        "Contact added (ID: {}...)",
        their_public_id.get(..16).unwrap_or(&their_public_id)
    ));
    display::info("They need to run 'vauchi sync' to see your contact request.");

    Ok(())
}

fn load_aha_tracker(config: &CliConfig) -> AhaMomentTracker {
    let path = config.data_dir.join("aha_tracker.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|json| AhaMomentTracker::from_json(&json).ok())
        .unwrap_or_default()
}

fn save_aha_tracker(config: &CliConfig, tracker: &AhaMomentTracker) {
    let path = config.data_dir.join("aha_tracker.json");
    if let Ok(json) = tracker.to_json() {
        let _ = crate::config::write_restricted(&path, json);
    }
}
