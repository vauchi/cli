// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exchange Command
//!
//! Generate and complete contact exchanges.

use std::fs;
use std::net::TcpStream;

use anyhow::{bail, Result};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{connect, Message, WebSocket};
use vauchi_core::contact_card::ContactCard;
use vauchi_core::exchange::{ExchangeQR, X3DH};
use vauchi_core::network::MockTransport;
use vauchi_core::sync::delta::CardDelta;
use vauchi_core::sync::{ContactSyncData, DeviceSyncOrchestrator, SyncItem};
use vauchi_core::{Contact, Identity, IdentityBackup, Vauchi, VauchiConfig};

use crate::config::CliConfig;
use crate::display;
use crate::protocol::{
    create_envelope, encode_message, EncryptedUpdate, ExchangeMessage, Handshake, MessagePayload,
};

/// Internal password for local identity storage.
const LOCAL_STORAGE_PASSWORD: &str = "vauchi-local-storage";

/// Opens Vauchi from the config and loads the identity.
fn open_vauchi(config: &CliConfig) -> Result<Vauchi<MockTransport>> {
    if !config.is_initialized() {
        bail!("Vauchi not initialized. Run 'vauchi init <name>' first.");
    }

    let wb_config = VauchiConfig::with_storage_path(config.storage_path())
        .with_relay_url(&config.relay_url)
        .with_storage_key(config.storage_key()?);

    let mut wb = Vauchi::new(wb_config)?;

    // Load identity from file
    let backup_data = fs::read(config.identity_path())?;
    let backup = IdentityBackup::new(backup_data);
    let identity = Identity::import_backup(&backup, LOCAL_STORAGE_PASSWORD)?;
    wb.set_identity(identity)?;

    Ok(wb)
}

/// Sends handshake message to relay.
fn send_handshake(
    socket: &mut WebSocket<MaybeTlsStream<TcpStream>>,
    client_id: &str,
) -> Result<()> {
    let handshake = Handshake {
        client_id: client_id.to_string(),
        device_id: None, // No device sync needed for exchange
    };
    let envelope = create_envelope(MessagePayload::Handshake(handshake));
    let data = encode_message(&envelope).map_err(|e| anyhow::anyhow!(e))?;
    socket.send(Message::Binary(data))?;
    Ok(())
}

/// Sends an exchange message to a recipient via the relay.
fn send_exchange_message(
    config: &CliConfig,
    our_identity: &Identity,
    recipient_id: &str,
    ephemeral_public: &[u8; 32],
) -> Result<()> {
    // Connect to relay
    let (mut socket, _) = connect(&config.relay_url)?;

    // Send handshake
    let our_id = our_identity.public_id();
    send_handshake(&mut socket, &our_id)?;

    // Create exchange message with the ephemeral key from X3DH
    let exchange_msg = ExchangeMessage::new(
        our_identity.signing_public_key(),
        ephemeral_public,
        our_identity.display_name(),
    );

    // Create encrypted update (using exchange message as ciphertext)
    let update = EncryptedUpdate {
        recipient_id: recipient_id.to_string(),
        sender_id: our_id.clone(),
        ciphertext: exchange_msg.to_bytes(),
    };

    let envelope = create_envelope(MessagePayload::EncryptedUpdate(update));
    let data = encode_message(&envelope).map_err(|e| anyhow::anyhow!(e))?;
    socket.send(Message::Binary(data))?;

    // Wait briefly for acknowledgment
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Close connection
    let _ = socket.close(None);

    Ok(())
}

/// Sends an initial encrypted card update to establish the responder's send chain.
///
/// This is critical for the Double Ratchet protocol: the responder cannot send
/// messages until they receive at least one message from the initiator. By sending
/// our card immediately after the exchange, we ensure both parties can send.
fn send_initial_card_update(
    config: &CliConfig,
    wb: &Vauchi<MockTransport>,
    identity: &Identity,
    contact_id: &str,
    recipient_id: &str,
) -> Result<()> {
    // Load our own card
    let our_card = wb
        .storage()
        .load_own_card()?
        .ok_or_else(|| anyhow::anyhow!("No own card found"))?;

    // Create a delta from empty card to our current card
    let empty_card = ContactCard::new(identity.display_name());
    let mut delta = CardDelta::compute(&empty_card, &our_card);
    delta.sign(identity);

    // Serialize delta
    let delta_bytes =
        serde_json::to_vec(&delta).map_err(|e| anyhow::anyhow!("Serialization error: {}", e))?;

    // Load ratchet and encrypt
    let (mut ratchet, is_initiator) = wb
        .storage()
        .load_ratchet_state(contact_id)?
        .ok_or_else(|| anyhow::anyhow!("Ratchet not found for contact"))?;

    let ratchet_msg = ratchet
        .encrypt(&delta_bytes)
        .map_err(|e| anyhow::anyhow!("Encryption error: {:?}", e))?;
    let encrypted = serde_json::to_vec(&ratchet_msg)
        .map_err(|e| anyhow::anyhow!("Serialization error: {}", e))?;

    // Save updated ratchet state
    wb.storage()
        .save_ratchet_state(contact_id, &ratchet, is_initiator)?;

    // Connect to relay and send
    let (mut socket, _) = connect(&config.relay_url)?;

    // Send handshake
    let our_id = identity.public_id();
    send_handshake(&mut socket, &our_id)?;

    // Create encrypted update message
    let update = EncryptedUpdate {
        recipient_id: recipient_id.to_string(),
        sender_id: our_id,
        ciphertext: encrypted,
    };

    let envelope = create_envelope(MessagePayload::EncryptedUpdate(update));
    let data = encode_message(&envelope).map_err(|e| anyhow::anyhow!(e))?;
    socket.send(Message::Binary(data))?;

    // Wait briefly for acknowledgment
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Close connection
    let _ = socket.close(None);

    Ok(())
}

/// Records a new contact addition for inter-device sync.
fn record_contact_added(wb: &Vauchi<MockTransport>, contact: &Contact) -> Result<()> {
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

    // Create ContactSyncData from the contact
    let contact_data = ContactSyncData::from_contact(contact);

    // Record the sync item
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
pub fn start(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;

    // Get our identity
    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("No identity found"))?;

    // Generate exchange QR
    let qr = ExchangeQR::generate(identity);
    let qr_data = qr.to_data_string();
    let qr_image = qr.to_qr_image_string();

    // Display
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
pub fn complete(config: &CliConfig, data: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    // Parse the exchange QR data
    let qr = ExchangeQR::from_data_string(data)?;

    // Check if expired
    if qr.is_expired() {
        bail!("This exchange QR code has expired. Ask them to generate a new one.");
    }

    // Get their public keys
    let their_signing_key = qr.public_key();
    let their_exchange_key = qr.exchange_key();
    let their_public_id = hex::encode(their_signing_key);

    // Check if we already have this contact
    if wb.get_contact(&their_public_id)?.is_some() {
        display::warning("You already have this contact.");
        return Ok(());
    }

    // Get our identity for X3DH
    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("No identity found"))?;
    let our_x3dh = identity.x3dh_keypair();

    // Perform X3DH as initiator to derive shared secret
    let (shared_secret, ephemeral_public) = X3DH::initiate(&our_x3dh, their_exchange_key)
        .map_err(|e| anyhow::anyhow!("X3DH key agreement failed: {:?}", e))?;

    // Create a placeholder contact
    // The real name will be received via sync
    let their_card = vauchi_core::ContactCard::new("New Contact");

    let contact = Contact::from_exchange(*their_signing_key, their_card, shared_secret.clone());
    let contact_id = contact.id().to_string();

    // Add the contact
    let contact_clone = contact.clone();
    wb.add_contact(contact)?;

    // Record for inter-device sync (if multiple devices)
    if let Err(e) = record_contact_added(&wb, &contact_clone) {
        display::warning(&format!("Could not record for device sync: {}", e));
    }

    // Initialize Double Ratchet as initiator for forward secrecy
    wb.create_ratchet_as_initiator(&contact_id, &shared_secret, *their_exchange_key)?;

    // Send initial encrypted card update to establish responder's send chain
    // This is critical: the responder cannot send until they receive a message from us
    match send_initial_card_update(config, &wb, identity, &contact_id, &their_public_id) {
        Ok(()) => {
            display::info("Sent initial card to enable bidirectional messaging");
        }
        Err(e) => {
            display::warning(&format!("Could not send initial card update: {}", e));
            display::info("The responder may not be able to send updates until you sync again.");
        }
    }

    // Send exchange message via relay with our ephemeral key
    println!("Sending exchange request via relay...");
    match send_exchange_message(config, identity, &their_public_id, &ephemeral_public) {
        Ok(()) => {
            display::success("Exchange request sent");
        }
        Err(e) => {
            display::warning(&format!("Could not send via relay: {}", e));
            display::info("The contact has been added locally.");
            display::info("Ask them to run 'vauchi sync' or share your QR code manually.");
        }
    }

    println!();
    display::success(&format!(
        "Contact added (ID: {}...)",
        &their_public_id[..16]
    ));
    display::info("They need to run 'vauchi sync' to see your contact request.");
    display::info("You should also run 'vauchi sync' to receive their card updates.");

    Ok(())
}
