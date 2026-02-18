// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Sync Command
//!
//! Synchronize with the relay server using async WebSocket I/O.

use std::fs;
use std::time::Duration;

use anyhow::{bail, Result};
use futures_util::{SinkExt, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use tokio_tungstenite::tungstenite::Message;
use vauchi_core::exchange::X3DH;
use vauchi_core::network::WebSocketTransport;
use vauchi_core::sync::{ContactSyncData, DeviceSyncOrchestrator, SyncItem};
use vauchi_core::{Contact, Identity, Vauchi, VauchiConfig};

use vauchi_core::aha_moments::{AhaMomentTracker, AhaMomentType};

use crate::config::CliConfig;
use crate::display;
use crate::protocol::{
    create_ack, create_device_sync_ack, create_envelope, decode_message, encode_message, AckStatus,
    DeviceSyncMessage, EncryptedUpdate, ExchangeMessage, MessagePayload,
};

/// Type alias for the async WebSocket stream.
type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Opens Vauchi from the config and loads the identity.
fn open_vauchi(config: &CliConfig) -> Result<Vauchi<WebSocketTransport>> {
    if !config.is_initialized() {
        bail!("Vauchi not initialized. Run 'vauchi init <name>' first.");
    }

    let wb_config = VauchiConfig::with_storage_path(config.storage_path())
        .with_relay_url(&config.relay_url)
        .with_storage_key(config.storage_key()?);

    let mut wb = Vauchi::with_transport_factory(wb_config, WebSocketTransport::new)?;

    let identity = config.import_local_identity()?;
    wb.set_identity(identity)?;

    Ok(wb)
}

/// Connect to relay server via async WebSocket with timeout.
async fn connect_to_relay(relay_url: &str) -> Result<WsStream> {
    let (ws_stream, _) = tokio::time::timeout(
        Duration::from_secs(5),
        tokio_tungstenite::connect_async(relay_url),
    )
    .await
    .map_err(|_| anyhow::anyhow!("Connection timed out"))?
    .map_err(|e| anyhow::anyhow!("WebSocket connection failed: {}", e))?;

    Ok(ws_stream)
}

/// Sends authenticated handshake message to relay.
async fn send_handshake(
    socket: &mut WsStream,
    identity: &Identity,
    device_id: Option<&str>,
) -> Result<()> {
    let handshake =
        crate::protocol::create_signed_handshake(identity, device_id.map(|s| s.to_string()));
    let envelope = create_envelope(MessagePayload::Handshake(handshake));
    let data = encode_message(&envelope).map_err(|e| anyhow::anyhow!(e))?;
    socket
        .send(Message::Binary(data))
        .await
        .map_err(|e| anyhow::anyhow!("Send error: {}", e))?;
    Ok(())
}

/// Sends an exchange response with our name to a contact.
pub async fn send_exchange_response(
    config: &CliConfig,
    our_identity: &Identity,
    recipient_id: &str,
) -> Result<()> {
    let mut socket = connect_to_relay(&config.relay_url).await?;

    send_handshake(&mut socket, our_identity, None).await?;

    let our_id = our_identity.public_id();

    // Create response message
    let exchange_msg = ExchangeMessage::new_response(
        &hex::encode(our_identity.signing_public_key()),
        &hex::encode(our_identity.exchange_public_key()),
        our_identity.display_name(),
    );

    // Create encrypted update
    let update = EncryptedUpdate {
        recipient_id: recipient_id.to_string(),
        sender_id: our_id,
        ciphertext: exchange_msg.to_bytes(),
    };

    let envelope = create_envelope(MessagePayload::EncryptedUpdate(update));
    let data = encode_message(&envelope).map_err(|e| anyhow::anyhow!(e))?;
    socket
        .send(Message::Binary(data))
        .await
        .map_err(|e| anyhow::anyhow!("Send error: {}", e))?;

    // Wait briefly for acknowledgment
    tokio::time::sleep(Duration::from_millis(100)).await;

    let _ = socket.close(None).await;

    Ok(())
}

/// Receives and processes pending messages from relay.
/// Returns: (total_received, exchange_messages, encrypted_card_updates, device_sync_messages)
#[allow(clippy::type_complexity)]
async fn receive_pending(
    socket: &mut WsStream,
    _wb: &Vauchi<WebSocketTransport>,
) -> Result<(
    usize,
    Vec<ExchangeMessage>,
    Vec<(String, Vec<u8>)>,
    Vec<DeviceSyncMessage>,
)> {
    let mut received = 0;
    let mut exchange_messages = Vec::new();
    let mut card_updates = Vec::new(); // (sender_id, ciphertext)
    let mut device_sync_messages = Vec::new();

    loop {
        // Use timeout to detect when no more messages are pending
        let msg = match tokio::time::timeout(Duration::from_secs(1), socket.next()).await {
            Ok(Some(Ok(msg))) => msg,
            Ok(Some(Err(_))) | Ok(None) => break,
            Err(_) => break, // Timeout — no more pending messages
        };

        match msg {
            Message::Binary(data) => {
                match decode_message(&data) {
                    Ok(envelope) => {
                        match envelope.payload {
                            MessagePayload::EncryptedUpdate(update) => {
                                received += 1;

                                // Check if this is an exchange message
                                if ExchangeMessage::is_exchange(&update.ciphertext) {
                                    if let Some(exchange) =
                                        ExchangeMessage::from_bytes(&update.ciphertext)
                                    {
                                        display::info(&format!(
                                            "Received exchange request from {}",
                                            exchange.display_name
                                        ));
                                        exchange_messages.push(exchange);
                                    }
                                } else {
                                    // This is an encrypted card update
                                    display::info(&format!(
                                        "Received encrypted update from {}",
                                        &update.sender_id[..8]
                                    ));
                                    card_updates
                                        .push((update.sender_id.clone(), update.ciphertext));
                                }

                                // Send acknowledgment
                                let ack = create_ack(
                                    &envelope.message_id,
                                    AckStatus::ReceivedByRecipient,
                                );
                                if let Ok(ack_data) = encode_message(&ack) {
                                    let _ = socket.send(Message::Binary(ack_data)).await;
                                }
                            }
                            MessagePayload::Acknowledgment(ack) => {
                                display::info(&format!(
                                    "Message {} acknowledged",
                                    &ack.message_id[..8]
                                ));
                            }
                            MessagePayload::DeviceSyncMessage(sync_msg) => {
                                received += 1;
                                display::info(&format!(
                                    "Received device sync from device {}...",
                                    &sync_msg.sender_device_id[..16]
                                ));
                                device_sync_messages.push(sync_msg);

                                // Send acknowledgment
                                let ack = create_device_sync_ack(&envelope.message_id, 0);
                                if let Ok(ack_data) = encode_message(&ack) {
                                    let _ = socket.send(Message::Binary(ack_data)).await;
                                }
                            }
                            MessagePayload::DeviceSyncAck(ack) => {
                                display::info(&format!(
                                    "Device sync {} acknowledged (version {})",
                                    &ack.message_id[..8],
                                    ack.synced_version
                                ));
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        display::warning(&format!("Failed to decode message: {}", e));
                    }
                }
            }
            Message::Ping(data) => {
                let _ = socket.send(Message::Pong(data)).await;
            }
            Message::Close(_) => {
                break;
            }
            _ => {
                // Ignore text messages, pongs, etc.
            }
        }
    }

    Ok((
        received,
        exchange_messages,
        card_updates,
        device_sync_messages,
    ))
}

/// Processes exchange messages and creates contacts.
fn process_exchange_messages(
    wb: &Vauchi<WebSocketTransport>,
    messages: Vec<ExchangeMessage>,
) -> Result<(usize, usize, Vec<String>)> {
    let mut added = 0;
    let mut updated = 0;
    let mut response_recipients = Vec::new();

    // Get our identity for X3DH
    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("No identity found"))?;
    let our_x3dh = identity.x3dh_keypair();

    for exchange in messages {
        // Parse the identity public key (signing key)
        let identity_key = match hex::decode(&exchange.identity_public_key) {
            Ok(bytes) if bytes.len() == 32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                arr
            }
            _ => {
                display::warning(&format!(
                    "Invalid identity key from {}",
                    exchange.display_name
                ));
                continue;
            }
        };

        let public_id = hex::encode(identity_key);

        // Check if this is a response to our exchange
        if exchange.is_response {
            // Update existing contact's name
            if let Some(mut contact) = wb.get_contact(&public_id)? {
                if contact.display_name() != exchange.display_name {
                    if let Err(e) = contact.set_display_name(&exchange.display_name) {
                        display::warning(&format!("Failed to update contact name: {:?}", e));
                        continue;
                    }
                    wb.update_contact(&contact)?;
                    display::success(&format!("Updated contact name: {}", exchange.display_name));
                    updated += 1;
                } else {
                    display::info(&format!(
                        "Contact {} already has correct name",
                        exchange.display_name
                    ));
                }
            } else {
                display::warning(&format!(
                    "Received response from unknown contact: {}",
                    exchange.display_name
                ));
            }
            continue;
        }

        // Parse the ephemeral public key (for X3DH)
        let ephemeral_key = match hex::decode(&exchange.ephemeral_public_key) {
            Ok(bytes) if bytes.len() == 32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                arr
            }
            _ => {
                display::warning(&format!(
                    "Invalid ephemeral key from {}",
                    exchange.display_name
                ));
                continue;
            }
        };

        // Check if we already have this contact
        if wb.get_contact(&public_id)?.is_some() {
            display::info(&format!("{} is already a contact", exchange.display_name));
            continue;
        }

        // Perform X3DH as responder to derive shared secret
        let shared_secret = match X3DH::respond(&our_x3dh, &identity_key, &ephemeral_key) {
            Ok(secret) => secret,
            Err(e) => {
                display::warning(&format!(
                    "X3DH key agreement failed for {}: {:?}",
                    exchange.display_name, e
                ));
                continue;
            }
        };

        // Create contact card
        let card = vauchi_core::ContactCard::new(&exchange.display_name);

        // Create contact
        let contact = Contact::from_exchange(identity_key, card, shared_secret.clone());
        let contact_id = contact.id().to_string();
        let contact_clone = contact.clone();
        wb.add_contact(contact)?;

        // Record for inter-device sync (if multiple devices)
        if let Err(e) = record_contact_for_device_sync(wb, &contact_clone) {
            display::warning(&format!("Could not record for device sync: {}", e));
        }

        // Initialize Double Ratchet as responder for forward secrecy
        // Recreate the X3DH keypair since we can't clone it
        let ratchet_dh = vauchi_core::exchange::X3DHKeyPair::from_bytes(our_x3dh.secret_bytes());
        if let Err(e) = wb.create_ratchet_as_responder(&contact_id, &shared_secret, ratchet_dh) {
            display::warning(&format!("Failed to initialize ratchet: {:?}", e));
        }

        display::success(&format!("Added contact: {}", exchange.display_name));
        added += 1;

        // Collect recipient IDs for async response sending
        response_recipients.push(public_id);
    }

    Ok((added, updated, response_recipients))
}

/// Processes encrypted card updates from contacts.
fn process_card_updates(
    wb: &Vauchi<WebSocketTransport>,
    updates: Vec<(String, Vec<u8>)>, // (sender_id, ciphertext)
) -> Result<usize> {
    let mut processed = 0;

    for (sender_id, ciphertext) in updates {
        // Get contact to display name
        let contact_name = match wb.get_contact(&sender_id)? {
            Some(c) => c.display_name().to_string(),
            None => {
                display::warning(&format!(
                    "Update from unknown contact: {}...",
                    &sender_id[..8]
                ));
                continue;
            }
        };

        // Process the encrypted update
        match wb.process_card_update(&sender_id, &ciphertext) {
            Ok(changed_fields) => {
                if changed_fields.is_empty() {
                    display::info(&format!("{} sent an update (no changes)", contact_name));
                } else {
                    display::success(&format!(
                        "{} updated: {}",
                        contact_name,
                        changed_fields.join(", ")
                    ));
                }
                processed += 1;
            }
            Err(e) => {
                display::warning(&format!(
                    "Failed to process update from {}: {:?}",
                    contact_name, e
                ));
            }
        }
    }

    Ok(processed)
}

/// Collects pending outbound updates as serialized envelopes (sync — no await).
/// Returns (update_id, serialized_envelope) pairs for async sending.
fn collect_pending_updates_data(
    wb: &Vauchi<WebSocketTransport>,
    our_id: &str,
) -> Result<Vec<(String, Vec<u8>, String)>> {
    let contacts = wb.list_contacts()?;
    let mut result = Vec::new();

    for contact in contacts {
        let pending = wb.storage().get_pending_updates(contact.id())?;

        for update in pending {
            if update.update_type != "card_delta" {
                continue;
            }

            let msg = EncryptedUpdate {
                recipient_id: contact.id().to_string(),
                sender_id: our_id.to_string(),
                ciphertext: update.payload,
            };

            let envelope = create_envelope(MessagePayload::EncryptedUpdate(msg));
            match encode_message(&envelope) {
                Ok(data) => {
                    result.push((update.id, data, contact.display_name().to_string()));
                }
                Err(e) => {
                    display::warning(&format!("Failed to encode update: {}", e));
                }
            }
        }
    }

    Ok(result)
}

/// Builds device sync envelopes (sync — no await).
fn build_device_sync_data(wb: &Vauchi<WebSocketTransport>, identity: &Identity) -> Vec<Vec<u8>> {
    match vauchi_core::sync::build_device_sync_envelopes(identity, wb.storage()) {
        Ok(e) => e,
        Err(e) => {
            display::warning(&format!("Failed to build device sync: {}", e));
            Vec::new()
        }
    }
}

/// Processes received device sync messages from other devices.
fn process_device_sync_messages(
    wb: &Vauchi<WebSocketTransport>,
    messages: Vec<DeviceSyncMessage>,
    identity: &Identity,
) -> Result<usize> {
    if messages.is_empty() {
        return Ok(0);
    }

    // Load device registry
    let registry = match wb.storage().load_device_registry()? {
        Some(r) => r,
        None => {
            display::warning("Received device sync but no registry found");
            return Ok(0);
        }
    };

    // Create orchestrator
    let mut orchestrator = DeviceSyncOrchestrator::new(
        wb.storage(),
        identity.create_device_info(),
        registry.clone(),
    );

    let mut processed = 0;

    for msg in messages {
        // Find sender device in registry
        let sender_device_id = match hex::decode(&msg.sender_device_id) {
            Ok(bytes) if bytes.len() == 32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                arr
            }
            _ => {
                display::warning("Invalid sender device ID in sync message");
                continue;
            }
        };

        let sender = match registry.find_device(&sender_device_id) {
            Some(d) => d,
            None => {
                display::warning(&format!(
                    "Sync from unknown device: {}...",
                    &msg.sender_device_id[..16]
                ));
                continue;
            }
        };

        // Decrypt the payload
        let payload = match orchestrator
            .decrypt_from_device(&sender.exchange_public_key, &msg.encrypted_payload)
        {
            Ok(p) => p,
            Err(e) => {
                display::warning(&format!(
                    "Failed to decrypt sync from {}: {:?}",
                    sender.device_name, e
                ));
                continue;
            }
        };

        // Parse sync items
        let items: Vec<SyncItem> = match serde_json::from_slice(&payload) {
            Ok(i) => i,
            Err(e) => {
                display::warning(&format!(
                    "Failed to parse sync items from {}: {}",
                    sender.device_name, e
                ));
                continue;
            }
        };

        // Process the items
        match orchestrator.process_incoming(items.clone()) {
            Ok(applied) => {
                if !applied.is_empty() {
                    display::info(&format!(
                        "Applied {} sync changes from {}",
                        applied.len(),
                        sender.device_name
                    ));

                    // Apply the changes to storage
                    for item in &applied {
                        if let Err(e) = apply_sync_item(wb, item) {
                            display::warning(&format!("Failed to apply sync item: {}", e));
                        }
                    }
                }
                processed += 1;
            }
            Err(e) => {
                display::warning(&format!(
                    "Failed to process sync from {}: {:?}",
                    sender.device_name, e
                ));
            }
        }

        // Mark as synced
        if let Err(e) = orchestrator.mark_synced(&sender_device_id, msg.version) {
            display::warning(&format!("Failed to mark sync complete: {:?}", e));
        }
    }

    Ok(processed)
}

/// Records a contact addition for inter-device sync.
fn record_contact_for_device_sync(
    wb: &Vauchi<WebSocketTransport>,
    contact: &Contact,
) -> Result<()> {
    // Try to load device registry - if none exists or only one device, skip
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

    orchestrator.record_local_change(item)?;

    Ok(())
}

/// Applies a single sync item to storage.
fn apply_sync_item(wb: &Vauchi<WebSocketTransport>, item: &SyncItem) -> Result<()> {
    match item {
        SyncItem::ContactAdded { contact_data, .. } => {
            // Check if contact already exists
            if wb.get_contact(&contact_data.id)?.is_none() {
                // Reconstruct contact from sync data
                let card: vauchi_core::ContactCard = serde_json::from_str(&contact_data.card_json)
                    .unwrap_or_else(|_| vauchi_core::ContactCard::new(&contact_data.display_name));
                let shared_key =
                    vauchi_core::crypto::SymmetricKey::from_bytes(contact_data.shared_key);
                let contact = Contact::from_exchange(contact_data.public_key, card, shared_key);
                wb.add_contact(contact)?;
                display::success(&format!(
                    "Synced new contact: {}",
                    contact_data.display_name
                ));
            }
        }
        SyncItem::ContactRemoved { contact_id, .. } => {
            if wb.get_contact(contact_id)?.is_some() {
                wb.remove_contact(contact_id)?;
                display::info(&format!("Removed contact: {}...", &contact_id[..8]));
            }
        }
        SyncItem::CardUpdated {
            field_label,
            new_value,
            ..
        } => {
            // Update own card field
            if let Some(mut card) = wb.storage().load_own_card()? {
                // Find and update the field, or add it
                if card.update_field_value(field_label, new_value).is_ok() {
                    wb.storage().save_own_card(&card)?;
                    display::info(&format!("Synced card field: {}", field_label));
                }
            }
        }
        SyncItem::VisibilityChanged {
            contact_id,
            field_label,
            is_visible,
            ..
        } => {
            // Update visibility for a specific field to a contact
            display::info(&format!(
                "Synced visibility for contact {}... field {} = {}",
                &contact_id[..8],
                field_label,
                is_visible
            ));
            // Note: Visibility is per-field per-contact, handled by labels system
            // This requires label management which is a more complex operation
        }
        SyncItem::LabelChange { .. } => {
            display::info("Synced label change");
        }
        SyncItem::ContactTrustChanged {
            contact_id,
            recovery_trusted,
            ..
        } => {
            display::info(&format!(
                "Synced trust change for contact {}... = {}",
                &contact_id[..8.min(contact_id.len())],
                recovery_trusted
            ));
        }
        SyncItem::DeletionScheduled { execute_at, .. } => {
            display::info(&format!(
                "Synced deletion schedule (executes at {})",
                execute_at
            ));
        }
        SyncItem::DeletionCancelled { .. } => {
            display::info("Synced deletion cancellation");
        }
    }
    Ok(())
}

/// Runs the sync command.
pub async fn run(config: &CliConfig) -> Result<()> {
    // ── Phase 1: Open Vauchi and extract data (sync, Vauchi is !Send) ──
    let wb = open_vauchi(config)?;

    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("No identity found"))?;
    let client_id = identity.public_id();
    let device_id_hex = hex::encode(identity.device_id());

    // Create a spinner for connection progress
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    spinner.set_message(format!("Connecting to {}...", config.relay_url));
    spinner.enable_steady_tick(Duration::from_millis(80));

    // ── Phase 2: Connect via async WebSocket ──
    let mut socket = connect_to_relay(&config.relay_url).await?;

    spinner.finish_and_clear();
    display::success("Connected");

    // Send authenticated handshake with device_id for inter-device sync
    send_handshake(&mut socket, identity, Some(&device_id_hex)).await?;

    // Small delay to let server send pending messages
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Create a spinner for receiving messages
    let recv_spinner = ProgressBar::new_spinner();
    recv_spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} {msg}")
            .unwrap(),
    );
    recv_spinner.set_message("Receiving pending messages...");
    recv_spinner.enable_steady_tick(Duration::from_millis(80));

    // Receive pending messages (async)
    let (received, exchange_messages, card_updates, device_sync_messages) =
        receive_pending(&mut socket, &wb).await?;
    recv_spinner.finish_and_clear();

    // ── Phase 3: Process received messages (sync, Vauchi scope) ──
    let (contacts_added, contacts_updated, response_recipients) =
        process_exchange_messages(&wb, exchange_messages)?;

    let cards_updated = process_card_updates(&wb, card_updates)?;

    let device_syncs_processed = process_device_sync_messages(&wb, device_sync_messages, identity)?;

    // Collect outbound data (sync)
    let pending_data = collect_pending_updates_data(&wb, &client_id)?;
    let device_envelopes = build_device_sync_data(&wb, identity);

    // ── Phase 4: Send outbound data (async) ──

    // Send exchange responses
    for recipient_id in &response_recipients {
        display::info(&format!("Sending our name to {}...", &recipient_id[..16]));
        match send_exchange_response(config, identity, recipient_id).await {
            Ok(()) => display::success("Response sent"),
            Err(e) => display::warning(&format!("Could not send response: {}", e)),
        }
    }

    // Send device sync envelopes
    let mut device_syncs_sent = 0;
    for data in device_envelopes {
        if socket.send(Message::Binary(data)).await.is_ok() {
            device_syncs_sent += 1;
        }
    }
    if device_syncs_sent > 0 {
        display::info(&format!(
            "Sent device sync to {} devices",
            device_syncs_sent
        ));
    }

    // Send pending updates
    let mut updates_sent = 0;
    let mut sent_ids = Vec::new();
    for (update_id, data, contact_name) in pending_data {
        if socket.send(Message::Binary(data)).await.is_ok() {
            sent_ids.push(update_id);
            updates_sent += 1;
            display::info(&format!("Sent update to {}", contact_name));
        }
    }

    // Close connection
    let _ = socket.close(None).await;

    // ── Phase 5: Cleanup sent updates (sync, Vauchi scope) ──
    for id in &sent_ids {
        let _ = wb.storage().delete_pending_update(id);
    }

    // Display results
    println!();
    let total_changes = received
        + contacts_added
        + contacts_updated
        + cards_updated
        + updates_sent
        + device_syncs_processed
        + device_syncs_sent;
    if total_changes > 0 {
        let mut summary = format!("Sync complete: {} received", received);
        if contacts_added > 0 {
            summary.push_str(&format!(", {} contacts added", contacts_added));
        }
        if contacts_updated > 0 {
            summary.push_str(&format!(", {} contacts updated", contacts_updated));
        }
        if cards_updated > 0 {
            summary.push_str(&format!(", {} cards updated", cards_updated));
        }
        if updates_sent > 0 {
            summary.push_str(&format!(", {} sent", updates_sent));
        }
        if device_syncs_processed > 0 {
            summary.push_str(&format!(
                ", {} device syncs received",
                device_syncs_processed
            ));
        }
        if device_syncs_sent > 0 {
            summary.push_str(&format!(", {} device syncs sent", device_syncs_sent));
        }
        display::success(&summary);
    } else {
        display::info("Sync complete: No new messages or pending updates");
    }

    // Check for aha moments
    let mut tracker = load_aha_tracker(config);
    if contacts_added > 0 {
        if let Some(moment) = tracker.try_trigger(AhaMomentType::FirstContactAdded) {
            display::display_aha_moment(&moment);
        }
    }
    if cards_updated > 0 {
        if let Some(moment) = tracker.try_trigger(AhaMomentType::FirstUpdateReceived) {
            display::display_aha_moment(&moment);
        }
    }
    if updates_sent > 0 {
        if let Some(moment) = tracker.try_trigger(AhaMomentType::FirstOutboundDelivered) {
            display::display_aha_moment(&moment);
        }
    }
    save_aha_tracker(config, &tracker);

    Ok(())
}

/// Load the aha moment tracker from the data directory.
fn load_aha_tracker(config: &CliConfig) -> AhaMomentTracker {
    let path = config.data_dir.join("aha_tracker.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|json| AhaMomentTracker::from_json(&json).ok())
        .unwrap_or_default()
}

/// Save the aha moment tracker to the data directory.
fn save_aha_tracker(config: &CliConfig, tracker: &AhaMomentTracker) {
    let path = config.data_dir.join("aha_tracker.json");
    if let Ok(json) = tracker.to_json() {
        let _ = fs::write(&path, json);
    }
}
