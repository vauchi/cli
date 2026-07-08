// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Device Commands
//!
//! Multi-device linking and management.

use std::fs;

use anyhow::{Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dialoguer::{Confirm, Input};
use vauchi_core::DeviceSyncOrchestrator;
use vauchi_core::exchange::{
    DeviceLinkQR, DeviceLinkResponder, DeviceLinkResponse, ProximityProof, compute_confirmation_mac,
};
use vauchi_core::sync::DeviceSyncPayload;
use vauchi_core::{Identity, Vauchi, VauchiConfig};

use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

/// Lists all linked devices.
pub fn list(config: &CliConfig, locale: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("No identity found"))?;

    let device_info = identity.device_info();

    println!();
    display::info(&format!(
        "Current device: {} (index {})",
        device_info.device_name(),
        device_info.device_index()
    ));
    println!(
        "  Device ID: {}",
        hex::encode(&device_info.device_id()[..8])
    );
    println!();

    match wb.storage().device().load_device_registry() {
        Ok(Some(registry)) => {
            println!("{}", display::t("cli.cmd.device.linked_devices", locale));
            println!("{}", "─".repeat(50));

            for (i, device) in registry.all_devices().iter().enumerate() {
                let status = if device.is_active() {
                    console::style("active").green()
                } else {
                    console::style("revoked").red()
                };

                let current = if device.device_id == *device_info.device_id() {
                    " (this device)"
                } else {
                    ""
                };

                println!(
                    "  {}. {} [{}]{}",
                    i + 1,
                    device.device_name,
                    status,
                    current
                );
                println!("     ID: {}...", hex::encode(&device.device_id[..8]));
            }
            println!("{}", "─".repeat(50));
            println!(
                "{}",
                display::tf(
                    "cli.cmd.device.total_devices",
                    locale,
                    &[("count", &registry.device_count().to_string())]
                )
            );
        }
        _ => {
            display::info("No device registry found. This is the only device.");
        }
    }

    Ok(())
}

/// Generates a QR code for linking a new device.
pub fn link(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;

    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("No identity found"))?;

    let registry = wb
        .storage()
        .device()
        .load_device_registry()?
        .unwrap_or_else(|| identity.initial_device_registry());

    display::info("Generating device link QR code...");
    println!();

    let initiator = identity.create_device_link_initiator(
        registry,
        vauchi_core::clock::SystemClock::shared().unix_seconds(),
    );
    let qr = initiator.qr();

    println!("{}", qr.to_qr_image_string());
    println!();

    let data_string = qr.to_data_string();
    let pending_link_path = config.data_dir.join(".pending_device_link");
    fs::create_dir_all(&config.data_dir)?;
    crate::config::write_restricted(&pending_link_path, &data_string)?;

    display::info("Device link data (for testing):");
    println!("  {}", data_string);
    println!();

    display::warning("This QR code expires in 5 minutes.");
    display::info("Scan this QR code with your new device using 'vauchi device join'");
    println!();

    display::info("After scanning, run 'vauchi device complete <request_data>' to finish linking.");

    Ok(())
}

/// Joins an existing identity by scanning/pasting the link QR data.
pub fn join(
    config: &CliConfig,
    qr_data: &str,
    device_name_arg: Option<&str>,
    yes: bool,
) -> Result<()> {
    if config.is_initialized() {
        display::warning("Vauchi is already initialized on this device.");

        if !yes {
            let confirm: String = Input::new()
                .with_prompt("This will replace your existing identity. Type 'yes' to continue")
                .interact_text()?;

            if confirm.to_lowercase() != "yes" {
                display::info("Join cancelled.");
                return Ok(());
            }
        }
    }

    let qr = DeviceLinkQR::from_data_string(qr_data)?;

    if qr.is_expired(vauchi_core::clock::SystemClock::shared().unix_seconds()) {
        bail!("Device link QR code has expired. Please generate a new one.");
    }

    display::success("QR code verified.");

    let device_name: String = if let Some(name) = device_name_arg {
        name.to_string()
    } else {
        Input::new()
            .with_prompt("Enter a name for this device")
            .default("New Device".to_string())
            .interact_text()?
    };

    let mut responder = DeviceLinkResponder::from_qr(
        qr,
        device_name.clone(),
        vauchi_core::clock::SystemClock::shared().unix_seconds(),
    )?;

    let encrypted_request =
        responder.create_request(vauchi_core::clock::SystemClock::shared().unix_seconds())?;

    let request_b64 = BASE64.encode(&encrypted_request);

    display::info("Send this request to the existing device:");
    println!();
    println!("  {}", request_b64);
    println!();

    display::info("On the existing device, run:");
    println!("  vauchi device complete {}", request_b64);
    println!();

    let link_key_path = config.data_dir.join(".pending_link_key");
    let device_name_path = config.data_dir.join(".pending_device_name");
    fs::create_dir_all(&config.data_dir)?;
    crate::config::write_restricted(&link_key_path, qr_data)?;
    crate::config::write_restricted(&device_name_path, &device_name)?;

    display::info("After the existing device responds, run:");
    println!("  vauchi device finish <response_data>");

    Ok(())
}

/// Completes the device linking on the existing device (processes request, sends response).
pub fn complete(config: &CliConfig, request_data: &str, auto_confirm: bool) -> Result<()> {
    let wb = open_vauchi(config)?;

    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("No identity found"))?;

    let pending_link_path = config.data_dir.join(".pending_device_link");
    if !pending_link_path.exists() {
        bail!("No pending device link. Run 'vauchi device link' first.");
    }

    let qr_data_string = fs::read_to_string(&pending_link_path)?;
    let saved_qr = DeviceLinkQR::from_data_string(&qr_data_string)?;

    if saved_qr.is_expired(vauchi_core::clock::SystemClock::shared().unix_seconds()) {
        let _ = fs::remove_file(&pending_link_path);
        bail!("Device link QR has expired. Please run 'vauchi device link' again.");
    }

    let registry = wb
        .storage()
        .device()
        .load_device_registry()?
        .unwrap_or_else(|| identity.initial_device_registry());

    let initiator = identity.restore_device_link_initiator(registry.clone(), saved_qr);

    let sync_orchestrator = DeviceSyncOrchestrator::new(
        wb.storage(),
        identity.create_device_info(vauchi_core::clock::SystemClock::shared().unix_seconds()),
        registry,
    );
    let sync_payload = sync_orchestrator
        .create_full_sync_payload()
        .map_err(|e| anyhow::anyhow!("Failed to create sync payload: {}", e))?;
    let sync_json = serde_json::to_string(&sync_payload)?;

    let encrypted_request = BASE64.decode(request_data)?;

    let (confirmation, request) = initiator.prepare_confirmation(&encrypted_request)?;

    display::info(&format!(
        "Device '{}' wants to link. Confirmation code: {}",
        confirmation.device_name, confirmation.confirmation_code
    ));

    let confirmed = if auto_confirm {
        display::info("Auto-confirming device link (--yes)");
        true
    } else {
        Confirm::new()
            .with_prompt("Does this confirmation code match the other device? Approve link?")
            .default(false)
            .interact()?
    };

    if !confirmed {
        anyhow::bail!("Device link cancelled by user");
    }

    // CLI uses manual data exchange (copy-paste) — construct manual confirmation proof
    let confirmation_code_mac =
        compute_confirmation_mac(initiator.qr().link_key(), &confirmation.confirmation_code);
    let proof = ProximityProof::ManualConfirmation {
        confirmation_code_mac,
        confirmed_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };

    let (encrypted_response, updated_registry, new_device) = initiator.confirm_link_with_sync(
        &request,
        &sync_json,
        &proof,
        vauchi_core::clock::SystemClock::shared().unix_seconds(),
    )?;

    wb.storage()
        .device()
        .save_device_registry(&updated_registry)?;

    let response_b64 = BASE64.encode(&encrypted_response);

    display::success(&format!(
        "Device '{}' approved for linking!",
        new_device.device_name()
    ));
    println!();

    display::info("Send this response to the new device:");
    println!();
    println!("  {}", response_b64);
    println!();

    display::info("On the new device, run:");
    println!("  vauchi device finish {}", response_b64);
    println!();

    let _ = fs::remove_file(&pending_link_path);

    display::success("Device linking initiated. Registry updated with new device.");

    Ok(())
}

/// Finishes the device join on the new device (processes response).
pub fn finish(config: &CliConfig, response_data: &str) -> Result<()> {
    let link_key_path = config.data_dir.join(".pending_link_key");
    let device_name_path = config.data_dir.join(".pending_device_name");

    if !link_key_path.exists() {
        bail!("No pending device link. Run 'vauchi device join' first.");
    }

    let qr_data = fs::read_to_string(&link_key_path)?;
    let device_name =
        fs::read_to_string(&device_name_path).unwrap_or_else(|_| "New Device".to_string());
    let qr = DeviceLinkQR::from_data_string(&qr_data)?;

    let encrypted_response = BASE64.decode(response_data)?;

    let response = DeviceLinkResponse::decrypt(&encrypted_response, qr.link_key())?;

    let identity = Identity::from_device_link(
        *response.master_seed(),
        response.display_name().to_string(),
        response.device_index(),
        device_name,
        vauchi_core::clock::SystemClock::shared().unix_seconds(),
    );

    config.save_local_identity(&identity)?;

    let wb_config = VauchiConfig::with_storage_path(config.storage_path())
        .with_relay_url(&config.relay_url)
        .with_storage_key(config.storage_key()?);
    let wb = Vauchi::new(wb_config)?;
    wb.storage()
        .device()
        .save_device_registry(response.registry())?;

    display::success(&format!("Joined identity: {}", response.display_name()));
    display::info(&format!("Device index: {}", response.device_index()));

    let _ = fs::remove_file(&link_key_path);
    let _ = fs::remove_file(&device_name_path);

    if !response.sync_payload_json().is_empty()
        && let Ok(payload) = DeviceSyncPayload::from_json(response.sync_payload_json())
    {
        let contact_count = payload.contact_count();

        let mut orchestrator = DeviceSyncOrchestrator::new(
            wb.storage(),
            identity.create_device_info(vauchi_core::clock::SystemClock::shared().unix_seconds()),
            response.registry().clone(),
        );

        if let Err(e) = orchestrator.apply_full_sync(payload) {
            display::warning(&format!("Failed to sync contacts: {}", e));
        } else if contact_count > 0 {
            display::success(&format!(
                "Synced {} contacts from existing device.",
                contact_count
            ));
        }
    }

    display::info("Device linking complete. Run 'vauchi sync' to fetch updates.");

    Ok(())
}

/// Revokes a device from the registry.
pub fn revoke(config: &CliConfig, device_id_prefix: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("No identity found"))?;

    // Find device by ID prefix (delegates registry lookup + search to core)
    let device = wb
        .find_device_by_prefix(device_id_prefix)?
        .ok_or_else(|| anyhow::anyhow!("Device not found: {}", device_id_prefix))?;

    let registry = wb
        .storage()
        .device()
        .load_device_registry()?
        .ok_or_else(|| anyhow::anyhow!("No device registry found"))?;

    if !device.is_active() {
        display::warning("Device is already revoked.");
        return Ok(());
    }

    if device.device_id == *identity.device_id() {
        bail!("Cannot revoke the current device. Use another device to revoke this one.");
    }

    let confirm: String = Input::new()
        .with_prompt(format!(
            "Revoke device '{}'? Type 'yes' to confirm",
            device.device_name
        ))
        .interact_text()?;

    if confirm.to_lowercase() != "yes" {
        display::info("Revocation cancelled.");
        return Ok(());
    }

    let mut updated_registry = registry.clone();
    updated_registry.revoke_device(
        &device.device_id,
        identity.signing_keypair(),
        wb.clock().unix_seconds(),
    )?;

    wb.storage()
        .device()
        .save_device_registry(&updated_registry)?;

    display::success(&format!(
        "Device '{}' has been revoked.",
        device.device_name
    ));
    display::info("The revocation will be propagated to contacts on next sync.");

    Ok(())
}

/// Shows device info for the current device.
pub fn info(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;

    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("No identity found"))?;

    let device_info = identity.device_info();

    println!();
    println!("{}", "─".repeat(50));
    println!("  {}", console::style("Device Information").bold().cyan());
    println!("{}", "─".repeat(50));
    println!();
    println!("  Name:        {}", device_info.device_name());
    println!("  Index:       {}", device_info.device_index());
    println!("  Device ID:   {}", hex::encode(device_info.device_id()));
    println!(
        "  Exchange Key: {}...",
        hex::encode(&device_info.exchange_public_key()[..16])
    );
    println!(
        "  Created:     {}",
        format_timestamp(device_info.created_at())
    );
    println!();
    println!("{}", "─".repeat(50));

    Ok(())
}

/// Formats a Unix timestamp as a human-readable string.
fn format_timestamp(ts: u64) -> String {
    use std::time::{Duration, UNIX_EPOCH};

    let d = UNIX_EPOCH + Duration::from_secs(ts);
    if let Ok(datetime) = d.duration_since(UNIX_EPOCH) {
        let secs = datetime.as_secs();
        // Simple formatting - in production use chrono
        format!("{} seconds since epoch", secs)
    } else {
        "Unknown".to_string()
    }
}
