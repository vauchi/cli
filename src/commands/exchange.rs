// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exchange Command
//!
//! Generate and complete contact exchanges.

use std::fs;
use std::net::{TcpListener, TcpStream};

use anyhow::{Result, bail};
use vauchi_core::Identity;
use vauchi_core::contact_card::ContactCard;
use vauchi_core::exchange::tcp_transport::TcpDirectTransport;
use vauchi_core::exchange::{
    ExchangeCommand, ExchangeEvent, ExchangeHardwareEvent, ExchangeQR, ExchangeSession,
    ExchangeState, ManualConfirmationVerifier, ProximityConfidence, UsbRole,
};
use vauchi_core::types::{AhaMomentTracker, AhaMomentType};

use crate::commands::common::{drain_activity_log, open_vauchi, register_activity_log_handler};
use crate::config::CliConfig;
use crate::display;

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

    // Capture exchange events (ContactAdded) for the activity log.
    let event_rx = register_activity_log_handler(&wb);

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
    wb.add_contact(contact)?;

    // Aha moment: first contact added
    let mut tracker = load_aha_tracker(config);
    if let Some(moment) =
        tracker.try_trigger_with_context(AhaMomentType::FirstContactAdded, their_name.to_string())
    {
        display::display_aha_moment(&moment);
    }
    save_aha_tracker(config, &tracker);

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

    drain_activity_log(&wb, event_rx);

    Ok(())
}

/// Performs a USB cable exchange as the initiator (desktop/TCP client side).
///
/// Connects to the phone's TCP address, exchanges payloads using the VXCH
/// framing protocol, then completes key agreement and contact creation.
pub fn usb_exchange(config: &CliConfig, address: &str) -> Result<()> {
    let mut wb = open_vauchi(config)?;
    let event_rx = register_activity_log_handler(&wb);

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

    let mut session =
        ExchangeSession::new_usb(identity_owned, our_card, verifier, UsbRole::Initiator);

    session.emit_initial_commands();
    let cmds = session.drain_commands();

    let (payload, is_initiator) = match &cmds[0] {
        ExchangeCommand::DirectSend {
            payload,
            is_initiator,
        } => (payload.clone(), *is_initiator),
        other => bail!("expected DirectSend command, got {:?}", other),
    };

    display::info(&format!("Connecting to {address}..."));

    let addr: std::net::SocketAddr = address
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid address '{}': {}", address, e))?;
    let stream = TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(10))
        .map_err(|e| anyhow::anyhow!("connection failed: {}", e))?;
    let mut transport = TcpDirectTransport::physical(stream);
    let their_payload = transport
        .exchange(&payload, is_initiator)
        .map_err(|e| anyhow::anyhow!("TCP exchange failed: {:?}", e))?;

    session
        .apply_hardware_event(ExchangeHardwareEvent::DirectPayloadReceived {
            data: their_payload,
        })
        .map_err(|e| anyhow::anyhow!("payload processing failed: {:?}", e))?;

    // Capture their_exchange_key before PerformKeyAgreement consumes the state
    let their_exchange_key = match session.state() {
        ExchangeState::AwaitingKeyAgreement {
            their_exchange_key, ..
        } => *their_exchange_key,
        _ => bail!("unexpected state after DirectPayloadReceived"),
    };

    session
        .apply(ExchangeEvent::PerformKeyAgreement)
        .map_err(|e| anyhow::anyhow!("key agreement failed: {:?}", e))?;

    session
        .apply(ExchangeEvent::ProximityCheckCompleted {
            confidence: ProximityConfidence::High,
        })
        .map_err(|e| anyhow::anyhow!("proximity check failed: {:?}", e))?;

    let shared_key = match session.state() {
        ExchangeState::AwaitingCardExchange { shared_key, .. } => shared_key.clone(),
        _ => bail!("unexpected state after key agreement"),
    };

    let their_name = session
        .their_display_name()
        .filter(|n| !n.is_empty())
        .unwrap_or("New Contact")
        .to_string();
    let their_card = ContactCard::new(&their_name);
    session
        .apply(ExchangeEvent::CompleteExchange(their_card))
        .map_err(|e| anyhow::anyhow!("complete exchange failed: {:?}", e))?;

    let contact = match session.state() {
        ExchangeState::Complete { contact } => *contact.clone(),
        _ => bail!("session not in Complete state"),
    };

    let contact_id = contact.id().to_string();
    wb.add_contact(contact)?;
    wb.create_ratchet_as_initiator(&contact_id, &shared_key, their_exchange_key)?;

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

    display::success(&format!("Contact '{}' added via USB exchange!", their_name));
    drain_activity_log(&wb, event_rx);
    Ok(())
}

/// Listens for a USB cable exchange as the responder (phone/TCP server side).
///
/// Binds to a TCP port, accepts one connection from the desktop initiator,
/// exchanges payloads using the VXCH framing protocol, then completes key
/// agreement and contact creation.
pub fn usb_listen(config: &CliConfig, port: u16) -> Result<()> {
    let mut wb = open_vauchi(config)?;
    let event_rx = register_activity_log_handler(&wb);

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

    let mut session =
        ExchangeSession::new_usb(identity_owned, our_card, verifier, UsbRole::Responder);

    session.emit_initial_commands();
    let cmds = session.drain_commands();

    let (payload, is_initiator) = match &cmds[0] {
        ExchangeCommand::DirectSend {
            payload,
            is_initiator,
        } => (payload.clone(), *is_initiator),
        other => bail!("expected DirectSend command, got {:?}", other),
    };

    let bind_addr = format!("0.0.0.0:{port}");
    display::info(&format!("Listening on {bind_addr}..."));

    let listener =
        TcpListener::bind(&bind_addr).map_err(|e| anyhow::anyhow!("bind failed: {}", e))?;
    let (stream, peer_addr) = listener
        .accept()
        .map_err(|e| anyhow::anyhow!("accept failed: {}", e))?;
    display::info(&format!("Connected from {peer_addr}"));

    let mut transport = TcpDirectTransport::physical(stream);
    let their_payload = transport
        .exchange(&payload, is_initiator)
        .map_err(|e| anyhow::anyhow!("TCP exchange failed: {:?}", e))?;

    session
        .apply_hardware_event(ExchangeHardwareEvent::DirectPayloadReceived {
            data: their_payload,
        })
        .map_err(|e| anyhow::anyhow!("payload processing failed: {:?}", e))?;

    // Capture their_exchange_key before PerformKeyAgreement consumes the state
    let their_exchange_key = match session.state() {
        ExchangeState::AwaitingKeyAgreement {
            their_exchange_key, ..
        } => *their_exchange_key,
        _ => bail!("unexpected state after DirectPayloadReceived"),
    };

    session
        .apply(ExchangeEvent::PerformKeyAgreement)
        .map_err(|e| anyhow::anyhow!("key agreement failed: {:?}", e))?;

    session
        .apply(ExchangeEvent::ProximityCheckCompleted {
            confidence: ProximityConfidence::High,
        })
        .map_err(|e| anyhow::anyhow!("proximity check failed: {:?}", e))?;

    let shared_key = match session.state() {
        ExchangeState::AwaitingCardExchange { shared_key, .. } => shared_key.clone(),
        _ => bail!("unexpected state after key agreement"),
    };

    let their_name = session
        .their_display_name()
        .filter(|n| !n.is_empty())
        .unwrap_or("New Contact")
        .to_string();
    let their_card = ContactCard::new(&their_name);
    session
        .apply(ExchangeEvent::CompleteExchange(their_card))
        .map_err(|e| anyhow::anyhow!("complete exchange failed: {:?}", e))?;

    let contact = match session.state() {
        ExchangeState::Complete { contact } => *contact.clone(),
        _ => bail!("session not in Complete state"),
    };

    let contact_id = contact.id().to_string();
    wb.add_contact(contact)?;
    // USB symmetric session: both sides use create_ratchet_as_initiator
    // with the peer's exchange public key (same symmetric DH shared secret).
    wb.create_ratchet_as_initiator(&contact_id, &shared_key, their_exchange_key)?;

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

    display::success(&format!("Contact '{}' added via USB exchange!", their_name));
    drain_activity_log(&wb, event_rx);
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
