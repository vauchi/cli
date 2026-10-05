// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;
use vauchi_core::contact_card::ContactAction;

use super::{action_label, execute_action, find_contact, opened_description};
use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

/// Opens a contact field in the system default application.
pub fn open_field(config: &CliConfig, contact_id_or_name: &str, field_label: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let contact = find_contact(&wb, contact_id_or_name)?;
    let contact_name = contact.display_name().to_string();

    let field = find_field(contact.card().fields(), field_label)
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
                Ok(_) => display::success(opened_field_description(&action)),
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

    let contact = find_contact(&wb, contact_id_or_name)?;
    let contact_name = contact.display_name().to_string();

    let fields = contact.card().fields();
    if fields.is_empty() {
        display::warning(&format!("{} has no contact fields", contact_name));
        return Ok(());
    }

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

    if !offers_action_menu(&actions) {
        return open_field(config, contact.id(), selected_field.label());
    }

    let action_items: Vec<String> = actions.iter().map(action_label).collect();

    let action_idx = Select::new()
        .with_prompt(format!("Action for {}", selected_field.label()))
        .items(&action_items)
        .default(0)
        .interact()?;

    execute_action(&actions[action_idx])
}

/// The field whose label matches, ignoring case.
fn find_field<'a>(
    fields: &'a [vauchi_core::contact_card::ContactField],
    label: &str,
) -> Option<&'a vauchi_core::contact_card::ContactField> {
    fields
        .iter()
        .find(|f| f.label().to_lowercase() == label.to_lowercase())
}

/// What the CLI reports once a field was opened.
fn opened_field_description(action: &ContactAction) -> &'static str {
    match action {
        ContactAction::CopyToClipboard => "Copied to clipboard",
        other => opened_description(other),
    }
}

/// Whether a field has more than its copy action, so the user picks one.
fn offers_action_menu(actions: &[ContactAction]) -> bool {
    actions.len() > 1
}

// INLINE_TEST_REQUIRED: Binary crate without lib.rs - tests cannot be external
#[cfg(test)]
mod tests {
    use vauchi_core::contact_card::{ContactAction, ContactField, FieldType};

    use super::{find_field, offers_action_menu, opened_field_description};

    // @internal
    #[test]
    fn a_field_is_found_by_label_in_any_case() {
        let fields = [
            ContactField::new(FieldType::Email, "Work", "a@work.ch", 0),
            ContactField::new(FieldType::Phone, "Mobile", "+41 79", 0),
        ];

        assert_eq!(
            find_field(&fields, "MOBILE").map(|f| f.value()),
            Some("+41 79")
        );
        assert!(find_field(&fields, "home").is_none());
    }

    // @internal
    #[test]
    fn opening_reports_by_action_and_copying_says_copied() {
        assert_eq!(
            opened_field_description(&ContactAction::Call("1".into())),
            "Opened dialer"
        );
        assert_eq!(
            opened_field_description(&ContactAction::CopyToClipboard),
            "Copied to clipboard"
        );
    }

    // @internal
    #[test]
    fn only_a_field_with_more_than_one_action_gets_a_menu() {
        let copy = ContactAction::CopyToClipboard;
        let call = ContactAction::Call("1".into());

        assert!(!offers_action_menu(&[]));
        assert!(!offers_action_menu(std::slice::from_ref(&copy)));
        assert!(offers_action_menu(&[call, copy]));
    }
}
