// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use vauchi_core::contact_card::ContactAction;

use super::{action_label, execute_action, find_contact};
use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

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
                        _ => "Opened",
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
