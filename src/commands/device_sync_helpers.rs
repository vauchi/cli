// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Device Sync Helpers
//!
//! Helper functions for recording inter-device sync items.
//! Used by card, contacts, and labels commands to propagate changes
//! across the user's own devices.

use anyhow::Result;
use vauchi_core::network::Transport;
use vauchi_core::sync::{DeviceSyncOrchestrator, SyncItem};
use vauchi_core::Vauchi;

/// Gets the current Unix timestamp.
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Records a card update for inter-device sync.
///
/// Call this after updating a card field to propagate the change to other devices.
pub fn record_card_update<T: Transport>(
    wb: &Vauchi<T>,
    field_label: &str,
    new_value: &str,
) -> Result<()> {
    // Try to load device registry - if none exists or only one device, skip
    let registry = match wb.storage().load_device_registry()? {
        Some(r) if r.device_count() > 1 => r,
        _ => return Ok(()), // No other devices to sync to
    };

    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("No identity found"))?;

    // Load orchestrator with existing state
    let mut orchestrator =
        DeviceSyncOrchestrator::load(wb.storage(), identity.create_device_info(), registry)
            .unwrap_or_else(|_| {
                DeviceSyncOrchestrator::new(
                    wb.storage(),
                    identity.create_device_info(),
                    identity.initial_device_registry(),
                )
            });

    let item = SyncItem::CardUpdated {
        field_label: field_label.to_string(),
        new_value: new_value.to_string(),
        timestamp: current_timestamp(),
    };

    orchestrator.record_local_change(item)?;

    Ok(())
}

/// Records a card field removal for inter-device sync.
///
/// Call this after removing a card field to propagate the deletion to other devices.
pub fn record_card_field_removed<T: Transport>(wb: &Vauchi<T>, field_label: &str) -> Result<()> {
    // Use empty string to indicate removal
    record_card_update(wb, field_label, "")
}

/// Records a contact removal for inter-device sync.
///
/// Call this after removing a contact to propagate the removal to other devices.
pub fn record_contact_removed<T: Transport>(wb: &Vauchi<T>, contact_id: &str) -> Result<()> {
    // Try to load device registry - if none exists or only one device, skip
    let registry = match wb.storage().load_device_registry()? {
        Some(r) if r.device_count() > 1 => r,
        _ => return Ok(()), // No other devices to sync to
    };

    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("No identity found"))?;

    let mut orchestrator =
        DeviceSyncOrchestrator::load(wb.storage(), identity.create_device_info(), registry)
            .unwrap_or_else(|_| {
                DeviceSyncOrchestrator::new(
                    wb.storage(),
                    identity.create_device_info(),
                    identity.initial_device_registry(),
                )
            });

    let item = SyncItem::ContactRemoved {
        contact_id: contact_id.to_string(),
        timestamp: current_timestamp(),
    };

    orchestrator.record_local_change(item)?;

    Ok(())
}

/// Records a visibility change for inter-device sync.
///
/// Call this after changing field visibility for a contact to propagate to other devices.
pub fn record_visibility_changed<T: Transport>(
    wb: &Vauchi<T>,
    contact_id: &str,
    field_label: &str,
    is_visible: bool,
) -> Result<()> {
    // Try to load device registry - if none exists or only one device, skip
    let registry = match wb.storage().load_device_registry()? {
        Some(r) if r.device_count() > 1 => r,
        _ => return Ok(()), // No other devices to sync to
    };

    let identity = wb
        .identity()
        .ok_or_else(|| anyhow::anyhow!("No identity found"))?;

    let mut orchestrator =
        DeviceSyncOrchestrator::load(wb.storage(), identity.create_device_info(), registry)
            .unwrap_or_else(|_| {
                DeviceSyncOrchestrator::new(
                    wb.storage(),
                    identity.create_device_info(),
                    identity.initial_device_registry(),
                )
            });

    let item = SyncItem::VisibilityChanged {
        contact_id: contact_id.to_string(),
        field_label: field_label.to_string(),
        is_visible,
        timestamp: current_timestamp(),
    };

    orchestrator.record_local_change(item)?;

    Ok(())
}
