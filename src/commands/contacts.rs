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

use crate::commands::common::{open_vauchi, open_vauchi_authenticated};
use crate::commands::device_sync_helpers::{record_contact_removed, record_visibility_changed};
use crate::config::CliConfig;
use crate::display;

/// Lists all contacts (respects auth mode — duress PIN shows decoys).
pub fn list(config: &CliConfig, pin: Option<&str>, offset: usize, limit: usize) -> Result<()> {
    let wb = open_vauchi_authenticated(config, pin)?;
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

/// Searches contacts by query (respects auth mode).
pub fn search(config: &CliConfig, pin: Option<&str>, query: &str) -> Result<()> {
    let wb = open_vauchi_authenticated(config, pin)?;
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

/// Exports a contact as vCard (.vcf format).
pub fn export(config: &CliConfig, id_or_name: &str, output_path: &str) -> Result<()> {
    use std::fs::File;
    use std::io::Write;
    use vauchi_core::contact_card::vcard::export_vcard;

    let wb = open_vauchi(config)?;

    // Find contact by ID or name
    let contact = find_contact(&wb, id_or_name)?;
    let contact_name = contact.display_name().to_string();

    // Generate vCard from contact's card
    let vcard_content = export_vcard(contact.card());

    // Write to file
    let mut file = File::create(output_path)?;
    file.write_all(vcard_content.as_bytes())?;

    display::success(&format!("Exported {} to {}", contact_name, output_path));

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
                        ContactAction::GetDirections(_) => "Opened directions",
                        ContactAction::CopyToClipboard => "Copied to clipboard",
                    };
                    display::success(action_desc);
                }
                Err(e) => {
                    display::error(&format!("Failed to open: {}", e));
                    println!();
                    println!("  Value: {}", field.value());
                    println!();
                    display::info("You can select and copy the value above manually.");
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
/// For fields with multiple actions (e.g. phone: Call/SMS/Copy), shows a
/// secondary action menu using to_secondary_actions().
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

    // Step 1: Select a field
    let field_items: Vec<String> = fields
        .iter()
        .map(|f| {
            let icon = display::field_icon(f.field_type());
            format!("{} {}: {}", icon, f.label(), f.value())
        })
        .collect();

    let field_idx = Select::new()
        .with_prompt(format!("Select field for {}", contact_name))
        .items(&field_items)
        .default(0)
        .interact()?;

    let selected_field = &fields[field_idx];
    let actions = selected_field.to_secondary_actions();

    // If only one action (CopyToClipboard), skip the action menu
    if actions.len() <= 1 {
        return open_field(config, contact.id(), selected_field.label());
    }

    // Step 2: Select an action from secondary actions
    let action_items: Vec<String> = actions.iter().map(action_label).collect();

    let action_idx = Select::new()
        .with_prompt(format!("Action for {}", selected_field.label()))
        .items(&action_items)
        .default(0)
        .interact()?;

    execute_action(&actions[action_idx])
}

/// Returns a human-readable label for a ContactAction.
fn action_label(action: &ContactAction) -> String {
    match action {
        ContactAction::Call(v) => format!("Call {}", v),
        ContactAction::SendSms(v) => format!("Send SMS to {}", v),
        ContactAction::SendEmail(v) => format!("Email {}", v),
        ContactAction::OpenUrl(v) => format!("Open {}", truncate_value(v, 40)),
        ContactAction::OpenMap(v) => format!("Open in Maps: {}", truncate_value(v, 30)),
        ContactAction::GetDirections(v) => format!("Get Directions to {}", truncate_value(v, 30)),
        ContactAction::CopyToClipboard => "Copy to Clipboard".to_string(),
    }
}

/// Truncates a string for display, appending "..." if too long.
fn truncate_value(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}

/// Executes a ContactAction by opening the appropriate URI.
fn execute_action(action: &ContactAction) -> Result<()> {
    let uri = match action {
        ContactAction::Call(v) => Some(format!("tel:{}", v)),
        ContactAction::SendSms(v) => Some(format!("sms:{}", v)),
        ContactAction::SendEmail(v) => Some(format!("mailto:{}", v)),
        ContactAction::OpenUrl(v) => Some(v.clone()),
        ContactAction::OpenMap(v) => {
            let encoded = url_encode_value(v);
            Some(format!(
                "https://www.openstreetmap.org/search?query={encoded}"
            ))
        }
        ContactAction::GetDirections(v) => {
            let encoded = url_encode_value(v);
            Some(format!(
                "https://www.openstreetmap.org/directions?route=&to={encoded}"
            ))
        }
        ContactAction::CopyToClipboard => None,
    };

    match uri {
        Some(uri_str) => match open::that(&uri_str) {
            Ok(_) => {
                let desc = match action {
                    ContactAction::Call(_) => "Opened dialer",
                    ContactAction::SendSms(_) => "Opened messaging",
                    ContactAction::SendEmail(_) => "Opened email client",
                    ContactAction::OpenUrl(_) => "Opened browser",
                    ContactAction::OpenMap(_) => "Opened maps",
                    ContactAction::GetDirections(_) => "Opened directions",
                    ContactAction::CopyToClipboard => unreachable!(),
                };
                display::success(desc);
                Ok(())
            }
            Err(e) => {
                display::error(&format!("Failed to open: {}", e));
                // Extract the raw value from the action for display
                let value = match action {
                    ContactAction::Call(v)
                    | ContactAction::SendSms(v)
                    | ContactAction::SendEmail(v) => v.as_str(),
                    ContactAction::OpenUrl(v)
                    | ContactAction::OpenMap(v)
                    | ContactAction::GetDirections(v) => v.as_str(),
                    ContactAction::CopyToClipboard => unreachable!(),
                };
                println!();
                println!("  Value: {}", value);
                println!();
                display::info("You can select and copy the value above manually.");
                Ok(())
            }
        },
        None => {
            display::info("Copy to clipboard is not available in CLI mode.");
            display::info("Use 'vauchi contacts show <name>' to view field values.");
            Ok(())
        }
    }
}

/// URL-encodes a value for use in map/directions URIs.
fn url_encode_value(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            '&' => "%26".to_string(),
            '?' => "%3F".to_string(),
            '#' => "%23".to_string(),
            _ if c.is_ascii_alphanumeric() || "-._~,+/".contains(c) => c.to_string(),
            _ => format!("%{:02X}", c as u32),
        })
        .collect()
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

/// Hides a contact from the default contact list.
pub fn hide_contact(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let contact = find_contact(&wb, id)?;
    let name = contact.display_name().to_string();

    if contact.is_hidden() {
        display::info(&format!("{} is already hidden", name));
        return Ok(());
    }

    wb.hide_contact(contact.id())?;
    display::success(&format!("Hidden {} from contact list", name));

    Ok(())
}

/// Unhides a previously hidden contact.
pub fn unhide_contact(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    // Need to search all contacts (including hidden) to find the hidden contact
    let contact = find_contact(&wb, id).or_else(|_| {
        // find_contact only searches visible contacts; try hidden list
        let hidden = wb.list_hidden_contacts()?;
        hidden
            .into_iter()
            .find(|c| c.id() == id || c.display_name().to_lowercase().contains(&id.to_lowercase()))
            .ok_or_else(|| anyhow::anyhow!("Contact '{}' not found", id))
    })?;
    let name = contact.display_name().to_string();

    if !contact.is_hidden() {
        display::info(&format!("{} is not hidden", name));
        return Ok(());
    }

    wb.unhide_contact(contact.id())?;
    display::success(&format!("Unhidden {} — now visible in contact list", name));

    Ok(())
}

/// Lists all hidden contacts.
pub fn list_hidden(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;

    let hidden = wb.list_hidden_contacts()?;

    if hidden.is_empty() {
        display::info("No hidden contacts.");
        return Ok(());
    }

    println!();
    println!("Hidden contacts ({}):", hidden.len());
    println!();

    display::display_contacts_table(&hidden);

    println!();
    display::info("Use 'vauchi contacts unhide-contact <id>' to restore.");
    println!();

    Ok(())
}

/// Blocks a contact (stops updates in both directions).
pub fn block(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let contact = find_contact(&wb, id)?;
    let name = contact.display_name().to_string();

    if contact.is_blocked() {
        display::info(&format!("{} is already blocked", name));
        return Ok(());
    }

    wb.block_contact(contact.id())?;
    display::success(&format!("Blocked {}", name));
    display::info("They will no longer receive your updates or send you updates.");

    Ok(())
}

/// Unblocks a previously blocked contact.
pub fn unblock(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let contact = find_contact(&wb, id)?;
    let name = contact.display_name().to_string();

    if !contact.is_blocked() {
        display::info(&format!("{} is not blocked", name));
        return Ok(());
    }

    wb.unblock_contact(contact.id())?;
    display::success(&format!("Unblocked {}", name));

    Ok(())
}

/// Lists all blocked contacts.
pub fn list_blocked(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;

    let blocked = wb.list_blocked_contacts()?;

    if blocked.is_empty() {
        display::info("No blocked contacts.");
        return Ok(());
    }

    println!();
    println!("Blocked contacts ({}):", blocked.len());
    println!();

    display::display_contacts_table(&blocked);

    println!();
    display::info("Use 'vauchi contacts unblock <id>' to unblock.");
    println!();

    Ok(())
}

/// Marks a contact as a favorite.
pub fn favorite(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let mut contact = find_contact(&wb, id)?;
    let name = contact.display_name().to_string();

    if contact.is_favorite() {
        display::info(&format!("{} is already a favorite", name));
        return Ok(());
    }

    contact.set_favorite(true);
    wb.update_contact(&contact)?;
    display::success(&format!("Marked {} as a favorite", name));

    Ok(())
}

/// Removes a contact from favorites.
pub fn unfavorite(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let mut contact = find_contact(&wb, id)?;
    let name = contact.display_name().to_string();

    if !contact.is_favorite() {
        display::info(&format!("{} is not a favorite", name));
        return Ok(());
    }

    contact.set_favorite(false);
    wb.update_contact(&contact)?;
    display::success(&format!("Removed {} from favorites", name));

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

/// Adds a personal note to a contact.
pub fn add_note(config: &CliConfig, id_or_name: &str, note_text: &str) -> Result<()> {
    use vauchi_core::crypto::encrypt;

    let wb = open_vauchi(config)?;

    // Find contact
    let contact = find_contact(&wb, id_or_name)?;
    let contact_id = contact.id().to_string();
    let contact_name = contact.display_name().to_string();

    // Encrypt note with contact's shared key
    let encrypted = encrypt(contact.shared_key(), note_text.as_bytes())?;

    // Save to storage
    wb.save_personal_notes(&contact_id, &encrypted)?;

    display::success(&format!("Added note to {}", contact_name));

    Ok(())
}

/// Shows the personal note for a contact.
pub fn show_note(config: &CliConfig, id_or_name: &str) -> Result<()> {
    use vauchi_core::crypto::decrypt;

    let wb = open_vauchi(config)?;

    // Find contact
    let contact = find_contact(&wb, id_or_name)?;
    let contact_id = contact.id().to_string();
    let contact_name = contact.display_name().to_string();

    // Load encrypted note
    let encrypted_opt = wb.load_personal_notes(&contact_id)?;

    match encrypted_opt {
        Some(encrypted) => {
            let decrypted = decrypt(contact.shared_key(), &encrypted)?;
            let note_text = String::from_utf8(decrypted)?;

            println!();
            println!("Note for {}:", contact_name);
            println!("{}", note_text);
            println!();
        }
        None => {
            display::info(&format!("No note for {}", contact_name));
        }
    }

    Ok(())
}

/// Edits the personal note for a contact.
pub fn edit_note(config: &CliConfig, id_or_name: &str, note_text: &str) -> Result<()> {
    use vauchi_core::crypto::encrypt;

    let wb = open_vauchi(config)?;

    // Find contact
    let contact = find_contact(&wb, id_or_name)?;
    let contact_id = contact.id().to_string();
    let contact_name = contact.display_name().to_string();

    // Encrypt new note with contact's shared key
    let encrypted = encrypt(contact.shared_key(), note_text.as_bytes())?;

    // Save to storage (overwrites existing)
    wb.save_personal_notes(&contact_id, &encrypted)?;

    display::success(&format!("Updated note for {}", contact_name));

    Ok(())
}

/// Deletes the personal note for a contact.
pub fn delete_note(config: &CliConfig, id_or_name: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    // Find contact
    let contact = find_contact(&wb, id_or_name)?;
    let contact_id = contact.id().to_string();
    let contact_name = contact.display_name().to_string();

    // Delete note from storage
    wb.delete_personal_notes(&contact_id)?;

    display::success(&format!("Deleted note for {}", contact_name));

    Ok(())
}

// ============================================================
// SP-12a: Contact Merge, Duplicates, and Limits
// ============================================================

/// Merges two contacts into one.
///
/// The first contact is the primary (keeps its name), and unique fields
/// from the second contact are added. The secondary contact is then removed.
///
/// # Examples
///
/// ```text
/// vauchi contacts merge "Alice" "Alice Work"
/// ```
pub fn merge(config: &CliConfig, contact1: &str, contact2: &str) -> Result<()> {
    use vauchi_core::contact::merge::merge_contacts;

    let wb = open_vauchi(config)?;

    // Find both contacts
    let primary = find_contact(&wb, contact1)?;
    let secondary = find_contact(&wb, contact2)?;

    // Prevent merging a contact with itself
    if primary.id() == secondary.id() {
        bail!("Cannot merge a contact with itself");
    }

    let primary_name = primary.display_name().to_string();
    let secondary_name = secondary.display_name().to_string();
    let secondary_id = secondary.id().to_string();

    // Show merge preview
    println!();
    println!("Merge preview:");
    println!("  Primary:   {} (fields kept)", primary_name);
    println!(
        "  Secondary: {} (unique fields added, then removed)",
        secondary_name
    );

    // Show which fields will be added from secondary
    let primary_labels: std::collections::HashSet<String> = primary
        .card()
        .fields()
        .iter()
        .map(|f| format!("{:?}:{}", f.field_type(), f.label()))
        .collect();

    let new_fields: Vec<_> = secondary
        .card()
        .fields()
        .iter()
        .filter(|f| {
            let sig = format!("{:?}:{}", f.field_type(), f.label());
            !primary_labels.contains(&sig)
        })
        .collect();

    if new_fields.is_empty() {
        println!("  No new fields to add from {}", secondary_name);
    } else {
        println!("  Fields to add from {}:", secondary_name);
        for field in &new_fields {
            println!(
                "    + {} ({}): {}",
                field.label(),
                display::field_icon(field.field_type()),
                field.value()
            );
        }
    }
    println!();

    // Perform the merge
    let merged = merge_contacts(&primary, &secondary);

    // Save merged contact
    wb.update_contact(&merged)?;

    // Remove secondary contact
    wb.remove_contact(&secondary_id)?;

    display::success(&format!(
        "Merged {} into {} ({} new fields added)",
        secondary_name,
        primary_name,
        new_fields.len()
    ));

    Ok(())
}

/// Lists potential duplicate contacts.
///
/// Finds contacts with high similarity scores and displays them,
/// excluding previously dismissed false positives.
///
/// # Examples
///
/// ```text
/// vauchi contacts duplicates
/// ```
pub fn duplicates(config: &CliConfig) -> Result<()> {
    use vauchi_core::contact::merge::{filter_dismissed, find_duplicates};

    let wb = open_vauchi(config)?;

    // Get all contacts (including hidden, for duplicate detection)
    let contacts = wb.list_contacts()?;

    if contacts.len() < 2 {
        display::info("Need at least 2 contacts to check for duplicates.");
        return Ok(());
    }

    // Find duplicates
    let all_duplicates = find_duplicates(&contacts);

    if all_duplicates.is_empty() {
        display::info("No potential duplicates found.");
        return Ok(());
    }

    // Filter out dismissed pairs
    let dismissed = wb.storage().load_dismissed_duplicates()?;
    let active_duplicates = filter_dismissed(all_duplicates, &dismissed);

    if active_duplicates.is_empty() {
        display::info("No potential duplicates found (all have been dismissed).");
        return Ok(());
    }

    println!();
    println!(
        "Potential duplicate contacts ({}):",
        active_duplicates.len()
    );
    println!();

    for (i, pair) in active_duplicates.iter().enumerate() {
        // Look up contact names
        let name1 = contacts
            .iter()
            .find(|c| c.id() == pair.id1)
            .map(|c| c.display_name().to_string())
            .unwrap_or_else(|| pair.id1[..8.min(pair.id1.len())].to_string());
        let name2 = contacts
            .iter()
            .find(|c| c.id() == pair.id2)
            .map(|c| c.display_name().to_string())
            .unwrap_or_else(|| pair.id2[..8.min(pair.id2.len())].to_string());

        let similarity_pct = (pair.similarity * 100.0) as u32;

        println!(
            "  {}. {} <-> {} ({}% similar)",
            i + 1,
            name1,
            name2,
            similarity_pct
        );
    }

    println!();
    display::info("Use 'vauchi contacts merge <contact1> <contact2>' to merge a pair.");
    display::info("Use 'vauchi contacts dismiss-duplicate <contact1> <contact2>' to dismiss a false positive.");
    println!();

    Ok(())
}

/// Dismisses a duplicate pair as a false positive.
///
/// The pair will no longer appear in the duplicates list.
///
/// # Examples
///
/// ```text
/// vauchi contacts dismiss-duplicate "Alice" "Alice Work"
/// ```
pub fn dismiss_duplicate(config: &CliConfig, contact1: &str, contact2: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    // Find both contacts
    let c1 = find_contact(&wb, contact1)?;
    let c2 = find_contact(&wb, contact2)?;

    // Prevent dismissing a contact with itself
    if c1.id() == c2.id() {
        bail!("Cannot dismiss a contact pair with itself");
    }

    let name1 = c1.display_name().to_string();
    let name2 = c2.display_name().to_string();

    // Dismiss in storage
    wb.storage().dismiss_duplicate(c1.id(), c2.id())?;

    display::success(&format!(
        "Dismissed duplicate pair: {} <-> {}",
        name1, name2
    ));
    display::info("This pair will no longer appear in the duplicates list.");
    display::info("Use 'vauchi contacts undismiss-duplicate <contact1> <contact2>' to undo.");

    Ok(())
}

/// Undismisses a previously dismissed duplicate pair.
///
/// The pair will appear again in the duplicates list if similarity
/// is still above threshold.
///
/// # Examples
///
/// ```text
/// vauchi contacts undismiss-duplicate "Alice" "Alice Work"
/// ```
pub fn undismiss_duplicate(config: &CliConfig, contact1: &str, contact2: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    // Find both contacts
    let c1 = find_contact(&wb, contact1)?;
    let c2 = find_contact(&wb, contact2)?;

    let name1 = c1.display_name().to_string();
    let name2 = c2.display_name().to_string();

    // Undismiss in storage
    wb.storage().undismiss_duplicate(c1.id(), c2.id())?;

    display::success(&format!(
        "Undismissed duplicate pair: {} <-> {}",
        name1, name2
    ));

    Ok(())
}

/// Shows or sets the contact limit.
///
/// Without `--set`, shows the current contact limit and usage.
/// With `--set N`, updates the maximum number of contacts allowed.
///
/// # Examples
///
/// ```text
/// vauchi contacts limit
/// vauchi contacts limit --set 500
/// ```
pub fn limit(config: &CliConfig, set_value: Option<usize>) -> Result<()> {
    let wb = open_vauchi(config)?;

    match set_value {
        Some(new_limit) => {
            // Validate the new limit
            if new_limit == 0 {
                bail!("Contact limit must be at least 1");
            }

            // Check if current count exceeds new limit
            let current_count = wb.contact_count().unwrap_or(0);
            if current_count > new_limit {
                display::warning(&format!(
                    "You have {} contacts, which exceeds the new limit of {}.",
                    current_count, new_limit
                ));
                display::info(
                    "Existing contacts will not be removed, but no new contacts can be added.",
                );
            }

            wb.storage().set_contact_limit(new_limit)?;
            display::success(&format!("Contact limit set to {}", new_limit));
        }
        None => {
            let max_contacts = wb.storage().get_contact_limit()?;
            let current_count = wb.contact_count().unwrap_or(0);

            println!();
            println!("Contact limit: {} / {}", current_count, max_contacts);

            if current_count >= max_contacts {
                display::warning("Contact limit reached. Remove contacts or increase the limit.");
            } else {
                let remaining = max_contacts - current_count;
                display::info(&format!("{} contact slots remaining.", remaining));
            }
            println!();
        }
    }

    Ok(())
}
