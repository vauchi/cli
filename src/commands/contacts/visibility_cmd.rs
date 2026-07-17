// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;

use super::{find_contact, find_field_id};
use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

/// Hides a field from a specific contact.
pub fn hide_field(config: &CliConfig, contact_id_or_name: &str, field_label: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let contact = find_contact(&wb, contact_id_or_name)?;
    let contact_name = contact.display_name().to_string();
    let contact_id = contact.id().to_string();

    let field_id = find_field_id(&wb, field_label)?;

    wb.set_contact_visibility_override_and_repropagate(&contact_id, &field_id, false)?;

    display::success(&format!(
        "Hidden '{}' field from {}",
        field_label, contact_name
    ));
    display::info("Changes will take effect on next sync.");

    Ok(())
}

/// Shows (unhides) a field to a specific contact.
pub fn unhide_field(config: &CliConfig, contact_id_or_name: &str, field_label: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let contact = find_contact(&wb, contact_id_or_name)?;
    let contact_name = contact.display_name().to_string();
    let contact_id = contact.id().to_string();

    let field_id = find_field_id(&wb, field_label)?;

    wb.set_contact_visibility_override_and_repropagate(&contact_id, &field_id, true)?;

    display::success(&format!(
        "'{}' field is now visible to {}",
        field_label, contact_name
    ));
    display::info("Changes will take effect on next sync.");

    Ok(())
}
