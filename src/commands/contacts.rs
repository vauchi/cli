// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Contacts Command
//!
//! List, view, and manage contacts.

use anyhow::{bail, Result};
use vauchi_core::contact_card::ContactAction;
use vauchi_core::network::MockTransport;
use vauchi_core::Vauchi;

use crate::commands::common::open_vauchi;
use crate::commands::device_sync_helpers::{record_contact_removed, record_visibility_changed};
use crate::config::CliConfig;
use crate::display;

/// Lists all contacts.
pub fn list(config: &CliConfig, offset: usize, limit: usize) -> Result<()> {
    let wb = open_vauchi(config)?;
    let total = wb.contact_count().unwrap_or(0);

    if total == 0 {
        display::info("No contacts yet. Exchange with someone using:");
        println!("  vauchi exchange start");
        return Ok(());
    }

    // Use core pagination API instead of manual slice
    let paginated = offset > 0 || limit > 0;
    let contacts = if paginated {
        wb.list_contacts_paginated(offset, limit)?
    } else {
        wb.list_contacts()?
    };

    println!();
    if paginated {
        println!(
            "Contacts (showing {}-{} of {}):",
            offset + 1,
            offset + contacts.len(),
            total
        );
    } else {
        println!("Contacts ({}):", total);
    }
    println!();

    display::display_contacts_table(&contacts);

    println!();

    Ok(())
}

/// Shows details for a specific contact.
pub fn show(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

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

/// Searches contacts by query.
pub fn search(config: &CliConfig, query: &str) -> Result<()> {
    let wb = open_vauchi(config)?;
    let results = wb.search_contacts(query)?;

    if results.is_empty() {
        display::info(&format!("No contacts matching '{}'", query));
        return Ok(());
    }

    println!();
    println!("Search results for '{}':", query);
    println!();

    for (i, contact) in results.iter().enumerate() {
        display::display_contact_summary(contact, i + 1);
    }

    println!();

    Ok(())
}

/// Removes a contact.
pub fn remove(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    // Get contact name before removing
    let contact = wb.get_contact(id)?;
    let name = contact.as_ref().map(|c| c.display_name().to_string());
    let contact_id = contact.as_ref().map(|c| c.id().to_string());

    if wb.remove_contact(id)? {
        display::success(&format!(
            "Removed contact: {}",
            name.unwrap_or_else(|| id.to_string())
        ));

        // Record for inter-device sync
        if let Some(cid) = contact_id {
            if let Err(e) = record_contact_removed(&wb, &cid) {
                display::warning(&format!("Failed to record for device sync: {}", e));
            }
        }
    } else {
        display::warning(&format!("Contact '{}' not found", id));
    }

    Ok(())
}

/// Marks a contact's fingerprint as verified.
pub fn verify(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    // Find contact by ID or name (supports partial ID prefixes)
    let contact = find_contact(&wb, id)?;
    let contact_id = contact.id().to_string();
    let name = contact.display_name().to_string();

    if contact.is_fingerprint_verified() {
        display::info(&format!("{} is already verified", name));
        return Ok(());
    }

    // Display fingerprints for manual comparison before marking verified
    println!();
    println!("  Their fingerprint ({}):", name);
    println!("  {}", contact.fingerprint());
    if let Ok(own_fp) = wb.own_fingerprint() {
        println!();
        println!("  Your fingerprint:");
        println!("  {}", own_fp);
    }
    println!();
    println!("  Compare these fingerprints in person before verifying.");
    println!();

    wb.verify_contact_fingerprint(&contact_id)?;
    display::success(&format!("Verified fingerprint for {}", name));

    Ok(())
}

/// Helper to find contact by ID or name
fn find_contact(wb: &Vauchi<MockTransport>, id_or_name: &str) -> Result<vauchi_core::Contact> {
    // Try exact ID match first
    if let Some(contact) = wb.get_contact(id_or_name)? {
        return Ok(contact);
    }

    // Use core fuzzy search (name substring + ID prefix matching)
    if let Some(contact) = wb
        .find_contact_fuzzy(id_or_name)
        .ok()
        .and_then(|results| results.into_iter().next())
    {
        return Ok(contact);
    }

    bail!("Contact '{}' not found", id_or_name)
}

/// Helper to find field ID by label in own card
fn find_field_id(wb: &Vauchi<MockTransport>, label: &str) -> Result<String> {
    let card = wb
        .own_card()?
        .ok_or_else(|| anyhow::anyhow!("No contact card found"))?;

    let field = card
        .fields()
        .iter()
        .find(|f| f.label() == label)
        .ok_or_else(|| anyhow::anyhow!("Field '{}' not found in your card", label))?;

    Ok(field.id().to_string())
}

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

/// Shows visibility rules for a specific contact.
pub fn show_visibility(config: &CliConfig, contact_id_or_name: &str) -> Result<()> {
    use vauchi_core::FieldVisibility;

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

/// Opens a contact field in the system default application.
pub fn open_field(config: &CliConfig, contact_id_or_name: &str, field_label: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    // Find contact
    let contact = find_contact(&wb, contact_id_or_name)?;
    let contact_name = contact.display_name().to_string();

    // Find the field by label
    let field = contact
        .card()
        .fields()
        .iter()
        .find(|f| f.label().to_lowercase() == field_label.to_lowercase())
        .ok_or_else(|| anyhow::anyhow!("Field '{}' not found for {}", field_label, contact_name))?;

    // Get URI using vauchi-core's secure URI builder
    let uri = field.to_uri();
    let action = field.to_action();

    match uri {
        Some(uri_str) => {
            display::info(&format!(
                "Opening {} for {}...",
                field.label(),
                contact_name
            ));

            match open::that(&uri_str) {
                Ok(_) => {
                    let action_desc = match action {
                        ContactAction::Call(_) => "Opened dialer",
                        ContactAction::SendSms(_) => "Opened messaging",
                        ContactAction::SendEmail(_) => "Opened email client",
                        ContactAction::OpenUrl(_) => "Opened browser",
                        ContactAction::OpenMap(_) => "Opened maps",
                        ContactAction::CopyToClipboard => "Copied to clipboard",
                    };
                    display::success(action_desc);
                }
                Err(e) => {
                    display::error(&format!("Failed to open: {}", e));
                    display::info(&format!("Value: {}", field.value()));
                }
            }
        }
        None => {
            display::warning(&format!(
                "Cannot open '{}' field - no action available",
                field.label()
            ));
            display::info(&format!("Value: {}", field.value()));
        }
    }

    Ok(())
}

/// Lists openable fields for a contact and lets user select one interactively.
pub fn open_interactive(config: &CliConfig, contact_id_or_name: &str) -> Result<()> {
    use dialoguer::Select;

    let wb = open_vauchi(config)?;

    // Find contact
    let contact = find_contact(&wb, contact_id_or_name)?;
    let contact_name = contact.display_name().to_string();

    let fields = contact.card().fields();
    if fields.is_empty() {
        display::warning(&format!("{} has no contact fields", contact_name));
        return Ok(());
    }

    // Build selection items
    let items: Vec<String> = fields
        .iter()
        .map(|f| {
            let action = f.to_action();
            let action_icon = match action {
                ContactAction::Call(_) => "phone",
                ContactAction::SendSms(_) => "sms",
                ContactAction::SendEmail(_) => "mail",
                ContactAction::OpenUrl(_) => "web",
                ContactAction::OpenMap(_) => "map",
                ContactAction::CopyToClipboard => "copy",
            };
            format!("[{}] {}: {}", action_icon, f.label(), f.value())
        })
        .collect();

    let selection = Select::new()
        .with_prompt(format!("Select field to open for {}", contact_name))
        .items(&items)
        .default(0)
        .interact()?;

    let selected_field = &fields[selection];
    open_field(config, contact.id(), selected_field.label())
}

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

/// Marks a contact as trusted for recovery.
pub fn trust(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let mut contact = wb
        .get_contact(id)?
        .or_else(|| {
            wb.search_contacts(id)
                .ok()
                .and_then(|results| results.into_iter().next())
        })
        .ok_or_else(|| anyhow::anyhow!("Contact '{}' not found", id))?;

    let name = contact.display_name().to_string();

    // Blocked contacts cannot be trusted for recovery
    if contact.is_blocked() {
        bail!("Blocked contacts cannot be trusted for recovery");
    }

    if contact.is_recovery_trusted() {
        display::info(&format!("{} is already trusted for recovery", name));
        return Ok(());
    }

    contact.trust_for_recovery();
    wb.update_contact(&contact)?;
    display::success(&format!("Marked {} as trusted for recovery", name));

    Ok(())
}

/// Removes recovery trust from a contact.
pub fn untrust(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let mut contact = wb
        .get_contact(id)?
        .or_else(|| {
            wb.search_contacts(id)
                .ok()
                .and_then(|results| results.into_iter().next())
        })
        .ok_or_else(|| anyhow::anyhow!("Contact '{}' not found", id))?;

    let name = contact.display_name().to_string();

    if !contact.is_recovery_trusted() {
        display::info(&format!("{} is not recovery-trusted", name));
        return Ok(());
    }

    contact.untrust_for_recovery();
    wb.update_contact(&contact)?;
    display::success(&format!("Removed recovery trust from {}", name));

    // Check if trusted count drops below threshold
    let readiness = wb.get_recovery_readiness()?;
    if !readiness.is_ready {
        display::warning(&format!(
            "Only {} trusted contact(s) remaining (recovery needs {})",
            readiness.trusted_count, readiness.threshold
        ));
    }

    Ok(())
}

/// Shows validation status for all of a contact's fields.
pub fn show_validation_status(config: &CliConfig, contact_id_or_name: &str) -> Result<()> {
    use vauchi_core::social::TrustLevel;

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
            TrustLevel::Unverified => "○",
            TrustLevel::LowConfidence => "◐",
            TrustLevel::PartialConfidence => "◑",
            TrustLevel::HighConfidence => "●",
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
