// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;

use super::find_contact;
use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

/// Validates a contact's field value (social proof).
pub fn validate_field(
    config: &CliConfig,
    contact_id_or_name: &str,
    field_label: &str,
) -> Result<()> {
    let wb = open_vauchi(config)?;

    // Find contact
    let contact = find_contact(&wb, contact_id_or_name)?;
    let contact_name = contact.display_name().to_string();
    let contact_id = contact.id().to_string();

    // Find the field by label
    let field = contact
        .card()
        .fields()
        .iter()
        .find(|f| f.label().to_lowercase() == field_label.to_lowercase())
        .ok_or_else(|| anyhow::anyhow!("Field '{}' not found for {}", field_label, contact_name))?;

    let field_id = field.id().to_string();
    let field_value = field.value().to_string();

    // Create validation
    let _validation = wb.validate_field(&contact_id, &field_id, &field_value)?;

    display::success(&format!(
        "Validated {} for {}: {}",
        field_label, contact_name, field_value
    ));
    display::info("Your validation has been recorded.");

    Ok(())
}

/// Revokes your validation of a contact's field.
pub fn revoke_validation(
    config: &CliConfig,
    contact_id_or_name: &str,
    field_label: &str,
) -> Result<()> {
    let wb = open_vauchi(config)?;

    // Find contact
    let contact = find_contact(&wb, contact_id_or_name)?;
    let contact_name = contact.display_name().to_string();
    let contact_id = contact.id().to_string();

    // Find the field by label
    let field = contact
        .card()
        .fields()
        .iter()
        .find(|f| f.label().to_lowercase() == field_label.to_lowercase())
        .ok_or_else(|| anyhow::anyhow!("Field '{}' not found for {}", field_label, contact_name))?;

    let field_id = field.id().to_string();

    // Revoke validation
    if wb.revoke_field_validation(&contact_id, &field_id)? {
        display::success(&format!(
            "Revoked validation for {} field of {}",
            field_label, contact_name
        ));
    } else {
        display::warning(&format!(
            "No validation found to revoke for {} field of {}",
            field_label, contact_name
        ));
    }

    Ok(())
}
