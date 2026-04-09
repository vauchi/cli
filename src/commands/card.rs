// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Card Command
//!
//! Manage your contact card.

use anyhow::{Result, bail};
use vauchi_core::{ContactField, FieldType};

use crate::commands::common::{drain_activity_log, open_vauchi, register_activity_log_handler};
use crate::config::CliConfig;
use crate::display;

/// Parses a field type string using core's alias table.
fn parse_field_type(s: &str) -> Result<(FieldType, Option<String>)> {
    FieldType::from_alias(s).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown field type: {}. Use: email, phone, website, address, social, custom",
            s
        )
    })
}

/// Shows the current contact card.
pub fn show(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;

    match wb.own_card()? {
        Some(card) => {
            if config.raw {
                crate::raw::print_json(&crate::raw::CardJson::from(&card))?;
            } else {
                display::display_card(&card);
            }
        }
        None => {
            display::warning("No contact card found. Create one with 'vauchi init'.");
        }
    }

    Ok(())
}

/// Adds a field to the contact card.
pub fn add(config: &CliConfig, field_type: &str, label: &str, value: &str) -> Result<()> {
    let wb = open_vauchi(config)?;
    let event_rx = register_activity_log_handler(&wb);

    let (ft, _label_hint) = parse_field_type(field_type)?;

    // Get old card for delta propagation
    let old_card = wb
        .own_card()?
        .ok_or_else(|| anyhow::anyhow!("No contact card found"))?;

    let field = ContactField::new(ft, label, value);
    wb.add_own_field(field)?;

    display::success(&format!("Added {} field '{}'", field_type, label));

    // Propagate update to contacts
    let new_card = wb.own_card()?.unwrap();
    let queued = wb.propagate_card_update(&old_card, &new_card)?;
    if queued > 0 {
        display::info(&format!("Update queued to {} contact(s)", queued));
    }

    drain_activity_log(&wb, event_rx);

    Ok(())
}

/// Interactively prompts for a social network and username, then adds the field.
///
/// Displays a numbered list of available social networks from the registry,
/// lets the user select by number, then prompts for the username.
pub fn add_social_interactive(config: &CliConfig) -> Result<()> {
    use dialoguer::{Input, Select};
    use vauchi_core::SocialNetworkRegistry;

    let wb = open_vauchi(config)?;
    let event_rx = register_activity_log_handler(&wb);

    // Ensure a card exists before prompting
    let old_card = wb
        .own_card()?
        .ok_or_else(|| anyhow::anyhow!("No contact card found. Run 'vauchi init' first."))?;

    // Load the social network registry
    let registry = SocialNetworkRegistry::with_defaults();
    let networks = registry.all();

    if networks.is_empty() {
        bail!("No social networks available in the registry");
    }

    // Build display items for the selector
    let items: Vec<String> = networks
        .iter()
        .map(|n| format!("{} ({})", n.display_name(), n.id()))
        .collect();

    println!();

    // Select allows arrow-key navigation through the list
    let selection = Select::new()
        .with_prompt("Select a social network")
        .items(&items)
        .default(0)
        .interact()?;

    let selected = networks[selection];
    let network_id = selected.id().to_string();
    let network_name = selected.display_name().to_string();

    // Prompt for username
    let username: String = Input::new()
        .with_prompt(format!("{} username", network_name))
        .interact_text()?;

    let username = username.trim().to_string();
    if username.is_empty() {
        bail!("Username cannot be empty");
    }

    // Show preview with profile URL
    if let Some(url) = registry.profile_url(&network_id, &username) {
        display::info(&format!("Profile URL: {}", url));
    }

    // Create and add the field (label = network id, value = username)
    let field = ContactField::new(FieldType::Social, &network_id, &username);
    wb.add_own_field(field)?;

    display::success(&format!(
        "Added social field '{}' with username '{}'",
        network_id, username
    ));

    // Propagate update to contacts
    let new_card = wb.own_card()?.unwrap();
    let queued = wb.propagate_card_update(&old_card, &new_card)?;
    if queued > 0 {
        display::info(&format!("Update queued to {} contact(s)", queued));
    }

    drain_activity_log(&wb, event_rx);

    Ok(())
}

/// Removes a field from the contact card.
pub fn remove(config: &CliConfig, label: &str) -> Result<()> {
    let wb = open_vauchi(config)?;
    let event_rx = register_activity_log_handler(&wb);

    // Get old card for delta propagation
    let old_card = wb
        .own_card()?
        .ok_or_else(|| anyhow::anyhow!("No contact card found"))?;

    if wb.remove_own_field(label)? {
        display::success(&format!("Removed field '{}'", label));

        // Propagate update to contacts
        let new_card = wb.own_card()?.unwrap();
        let queued = wb.propagate_card_update(&old_card, &new_card)?;
        if queued > 0 {
            display::info(&format!("Update queued to {} contact(s)", queued));
        }
    } else {
        display::warning(&format!("Field '{}' not found", label));
    }

    drain_activity_log(&wb, event_rx);

    Ok(())
}

/// Edits a field value.
pub fn edit(config: &CliConfig, label: &str, value: &str) -> Result<()> {
    let wb = open_vauchi(config)?;
    let event_rx = register_activity_log_handler(&wb);

    // Get current card (also serves as old card for delta)
    let old_card = wb
        .own_card()?
        .ok_or_else(|| anyhow::anyhow!("No contact card found"))?;

    // Find the field
    let field = old_card.fields().iter().find(|f| f.label() == label);

    match field {
        Some(f) => {
            // Remove old and add new
            wb.remove_own_field(label)?;
            let new_field = ContactField::new(f.field_type(), label, value);
            wb.add_own_field(new_field)?;

            display::success(&format!("Updated field '{}'", label));

            // Propagate update to contacts
            let new_card = wb.own_card()?.unwrap();
            let queued = wb.propagate_card_update(&old_card, &new_card)?;
            if queued > 0 {
                display::info(&format!("Update queued to {} contact(s)", queued));
            }
        }
        None => {
            display::warning(&format!("Field '{}' not found", label));
        }
    }

    drain_activity_log(&wb, event_rx);

    Ok(())
}

/// Edits the display name.
pub fn edit_name(config: &CliConfig, name: &str) -> Result<()> {
    let mut wb = open_vauchi(config)?;
    let event_rx = register_activity_log_handler(&wb);

    // Get old card for delta propagation
    let old_card = wb
        .own_card()?
        .ok_or_else(|| anyhow::anyhow!("No contact card found"))?;

    // Update display name
    wb.update_display_name(name)?;

    display::success(&format!("Display name updated to '{}'", name));

    // Propagate update to contacts
    let new_card = wb.own_card()?.unwrap();
    let queued = wb.propagate_card_update(&old_card, &new_card)?;
    if queued > 0 {
        display::info(&format!("Update queued to {} contact(s)", queued));
    }

    drain_activity_log(&wb, event_rx);

    Ok(())
}

// INLINE_TEST_REQUIRED: Binary crate without lib.rs - tests cannot be external
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // ====================================================================
    // Known-alias unit tests
    // ====================================================================

    #[test]
    fn test_parse_field_type_email_aliases() {
        assert_eq!(parse_field_type("email").unwrap().0, FieldType::Email);
        assert_eq!(parse_field_type("mail").unwrap().0, FieldType::Email);
        assert_eq!(parse_field_type("EMAIL").unwrap().0, FieldType::Email);
    }

    #[test]
    fn test_parse_field_type_phone_aliases() {
        assert_eq!(parse_field_type("phone").unwrap().0, FieldType::Phone);
        assert_eq!(parse_field_type("tel").unwrap().0, FieldType::Phone);
        assert_eq!(parse_field_type("telephone").unwrap().0, FieldType::Phone);
    }

    #[test]
    fn test_parse_field_type_unknown_returns_error() {
        assert!(parse_field_type("unknown").is_err());
        assert!(parse_field_type("").is_err());
    }

    // ====================================================================
    // Property-Based Tests (CC-04, CC-14)
    // ====================================================================

    /// All known aliases mapped to their expected FieldType.
    fn all_valid_aliases() -> Vec<(&'static str, FieldType)> {
        vec![
            ("email", FieldType::Email),
            ("mail", FieldType::Email),
            ("phone", FieldType::Phone),
            ("tel", FieldType::Phone),
            ("telephone", FieldType::Phone),
            ("website", FieldType::Website),
            ("web", FieldType::Website),
            ("url", FieldType::Website),
            ("address", FieldType::Address),
            ("addr", FieldType::Address),
            ("home", FieldType::Address),
            ("birthday", FieldType::Birthday),
            ("bday", FieldType::Birthday),
            ("dob", FieldType::Birthday),
            ("social", FieldType::Social),
            ("twitter", FieldType::Social),
            ("x", FieldType::Social),
            ("instagram", FieldType::Social),
            ("ig", FieldType::Social),
            ("linkedin", FieldType::Social),
            ("github", FieldType::Social),
            ("gh", FieldType::Social),
            ("custom", FieldType::Custom),
            ("other", FieldType::Custom),
            ("note", FieldType::Custom),
        ]
    }

    proptest! {
        /// Case-insensitive: any mixed-case variant of a valid alias is accepted.
        #[test]
        fn prop_parse_field_type_case_insensitive(
            idx in 0usize..25,
            bits in proptest::collection::vec(any::<bool>(), 20),
        ) {
            let aliases = all_valid_aliases();
            let (alias, expected_type) = &aliases[idx];
            // Apply random casing
            let mixed: String = alias.chars().enumerate().map(|(i, c)| {
                if *bits.get(i).unwrap_or(&false) {
                    c.to_uppercase().next().unwrap()
                } else {
                    c
                }
            }).collect();

            let result = parse_field_type(&mixed);
            prop_assert!(result.is_ok(), "Should accept '{}' (from alias '{}')", mixed, alias);
            prop_assert_eq!(
                std::mem::discriminant(&result.unwrap().0),
                std::mem::discriminant(expected_type),
            );
        }

        /// Arbitrary non-alias strings always produce an error.
        #[test]
        fn prop_parse_field_type_rejects_unknown(s in "\\PC{0,100}") {
            let known: Vec<&str> = vec![
                "email", "mail", "phone", "tel", "telephone",
                "website", "web", "url", "address", "addr", "home",
                "birthday", "bday", "dob",
                "social", "twitter", "x", "instagram", "ig",
                "linkedin", "github", "gh",
                "custom", "other", "note",
            ];
            if !known.contains(&s.to_lowercase().as_str()) {
                prop_assert!(parse_field_type(&s).is_err());
            }
        }

        /// Adversarial inputs (CC-14): never panics.
        #[test]
        fn prop_parse_field_type_never_panics(
            s in prop::string::string_regex("(.|\n){0,200}").unwrap()
        ) {
            // allow(zero_assertions): No-panic fuzz test
            let _ = parse_field_type(&s);
        }
    }
}
