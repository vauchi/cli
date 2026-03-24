// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use vauchi_core::FieldVisibility;

use super::find_contact;
use crate::commands::common::{open_vauchi, open_vauchi_authenticated};
use crate::config::CliConfig;
use crate::display;

/// Shows details for a specific contact (respects auth mode).
pub fn show(config: &CliConfig, pin: Option<&str>, id: &str) -> Result<()> {
    let wb = open_vauchi_authenticated(config, pin)?;

    // Try to find by ID first, then by name
    let contact = wb.get_contact(id)?.or_else(|| {
        // Search by name
        wb.search_contacts(id)
            .ok()
            .and_then(|results| results.into_iter().next())
    });

    match contact {
        Some(c) => {
            display::display_contact_details(&c);
        }
        None => {
            display::warning(&format!("Contact '{}' not found", id));
        }
    }

    Ok(())
}

/// Shows visibility rules for a specific contact.
pub fn show_visibility(config: &CliConfig, contact_id_or_name: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    // Find contact
    let contact = find_contact(&wb, contact_id_or_name)?;
    let contact_name = contact.display_name().to_string();

    // Get our card fields
    let card = wb
        .own_card()?
        .ok_or_else(|| anyhow::anyhow!("No contact card found"))?;

    println!();
    println!("Visibility rules for {}:", contact_name);
    println!();

    if card.fields().is_empty() {
        display::info("No fields in your card.");
        return Ok(());
    }

    let rules = contact.visibility_rules();
    let mut has_custom_rules = false;

    for field in card.fields() {
        let visibility = rules.get(field.id());
        let status = match visibility {
            FieldVisibility::Everyone => "✓ visible",
            FieldVisibility::Nobody => "✗ hidden",
            FieldVisibility::Contacts(allowed) => {
                if allowed.contains(&contact.id().to_string()) {
                    "✓ visible (restricted)"
                } else {
                    "✗ hidden (restricted)"
                }
            }
        };

        if !matches!(visibility, FieldVisibility::Everyone) {
            has_custom_rules = true;
        }

        println!("  {} {}: {}", status, field.label(), field.value());
    }

    if !has_custom_rules {
        println!();
        display::info("All fields are visible to this contact (default).");
    }

    println!();

    Ok(())
}

/// Shows validation status for all of a contact's fields.
pub fn show_validation_status(config: &CliConfig, contact_id_or_name: &str) -> Result<()> {
    use vauchi_core::social::ValidationConfidence;

    let wb = open_vauchi(config)?;

    // Find contact
    let contact = find_contact(&wb, contact_id_or_name)?;
    let contact_name = contact.display_name().to_string();
    let contact_id = contact.id().to_string();

    println!();
    println!("Validation status for {}:", contact_name);
    println!();

    let fields = contact.card().fields();
    if fields.is_empty() {
        display::info("No fields in contact's card.");
        return Ok(());
    }

    for field in fields {
        let status = wb.get_field_validation_status(&contact_id, field.id(), field.value())?;

        let trust_indicator = match status.trust_level {
            ValidationConfidence::Unverified => "○",
            ValidationConfidence::LowConfidence => "◐",
            ValidationConfidence::PartialConfidence => "◑",
            ValidationConfidence::HighConfidence => "●",
        };

        let validated_by_me = if status.validated_by_me { " (you)" } else { "" };

        println!(
            "  {} {}: {} [{} validations{}]",
            trust_indicator,
            field.label(),
            field.value(),
            status.count,
            validated_by_me
        );
    }

    println!();
    display::info("Legend: ○ unverified, ◐ low, ◑ partial, ● high confidence");
    println!();

    Ok(())
}
