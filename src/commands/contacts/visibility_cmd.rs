// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;

use super::{find_contact, find_field_id};
use crate::commands::common::open_vauchi;
use crate::commands::device_sync_helpers::record_visibility_changed;
use crate::config::CliConfig;
use crate::display;

/// Hides a field from a specific contact.
pub fn hide_field(config: &CliConfig, contact_id_or_name: &str, field_label: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    // Find contact
    let mut contact = find_contact(&wb, contact_id_or_name)?;
    let contact_name = contact.display_name().to_string();
    let contact_id = contact.id().to_string();

    // Find field ID by label
    let field_id = find_field_id(&wb, field_label)?;

    // Set visibility to nobody for this field
    contact.visibility_rules_mut().set_nobody(&field_id);
    wb.update_contact(&contact)?;

    display::success(&format!(
        "Hidden '{}' field from {}",
        field_label, contact_name
    ));
    display::info("Changes will take effect on next sync.");

    // Record for inter-device sync
    if let Err(e) = record_visibility_changed(&wb, &contact_id, field_label, false) {
        display::warning(&format!("Failed to record for device sync: {}", e));
    }

    Ok(())
}

/// Shows (unhides) a field to a specific contact.
pub fn unhide_field(config: &CliConfig, contact_id_or_name: &str, field_label: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    // Find contact
    let mut contact = find_contact(&wb, contact_id_or_name)?;
    let contact_name = contact.display_name().to_string();
    let contact_id = contact.id().to_string();

    // Find field ID by label
    let field_id = find_field_id(&wb, field_label)?;

    // Set visibility to everyone for this field
    contact.visibility_rules_mut().set_everyone(&field_id);
    wb.update_contact(&contact)?;

    display::success(&format!(
        "'{}' field is now visible to {}",
        field_label, contact_name
    ));
    display::info("Changes will take effect on next sync.");

    // Record for inter-device sync
    if let Err(e) = record_visibility_changed(&wb, &contact_id, field_label, true) {
        display::warning(&format!("Failed to record for device sync: {}", e));
    }

    Ok(())
}
