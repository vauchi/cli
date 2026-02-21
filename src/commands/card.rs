// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Card Command
//!
//! Manage your contact card.

use anyhow::{bail, Result};
use vauchi_core::{ContactField, FieldType};

use crate::commands::common::open_vauchi;
use crate::commands::device_sync_helpers::{record_card_field_removed, record_card_update};
use crate::config::CliConfig;
use crate::display;

/// Parses a field type string.
fn parse_field_type(s: &str) -> Result<FieldType> {
    match s.to_lowercase().as_str() {
        "email" | "mail" => Ok(FieldType::Email),
        "phone" | "tel" | "telephone" => Ok(FieldType::Phone),
        "website" | "web" | "url" => Ok(FieldType::Website),
        "address" | "addr" | "home" => Ok(FieldType::Address),
        "social" | "twitter" | "instagram" | "linkedin" => Ok(FieldType::Social),
        "custom" | "other" | "note" => Ok(FieldType::Custom),
        _ => bail!(
            "Unknown field type: {}. Use: email, phone, website, address, social, custom",
            s
        ),
    }
}

/// Shows the current contact card.
pub fn show(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;

    match wb.own_card()? {
        Some(card) => {
            display::display_card(&card);
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
    let ft = parse_field_type(field_type)?;

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

    // Record for inter-device sync
    if let Err(e) = record_card_update(&wb, label, value) {
        display::warning(&format!("Failed to record for device sync: {}", e));
    }

    Ok(())
}

/// Removes a field from the contact card.
pub fn remove(config: &CliConfig, label: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

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

        // Record for inter-device sync
        if let Err(e) = record_card_field_removed(&wb, label) {
            display::warning(&format!("Failed to record for device sync: {}", e));
        }
    } else {
        display::warning(&format!("Field '{}' not found", label));
    }

    Ok(())
}

/// Edits a field value.
pub fn edit(config: &CliConfig, label: &str, value: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

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

            // Record for inter-device sync
            if let Err(e) = record_card_update(&wb, label, value) {
                display::warning(&format!("Failed to record for device sync: {}", e));
            }
        }
        None => {
            display::warning(&format!("Field '{}' not found", label));
        }
    }

    Ok(())
}

/// Edits the display name.
pub fn edit_name(config: &CliConfig, name: &str) -> Result<()> {
    let mut wb = open_vauchi(config)?;

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

    // Record for inter-device sync (display_name is a special field)
    if let Err(e) = record_card_update(&wb, "_display_name", name) {
        display::warning(&format!("Failed to record for device sync: {}", e));
    }

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
        assert!(matches!(
            parse_field_type("email").unwrap(),
            FieldType::Email
        ));
        assert!(matches!(
            parse_field_type("mail").unwrap(),
            FieldType::Email
        ));
        assert!(matches!(
            parse_field_type("EMAIL").unwrap(),
            FieldType::Email
        ));
    }

    #[test]
    fn test_parse_field_type_phone_aliases() {
        assert!(matches!(
            parse_field_type("phone").unwrap(),
            FieldType::Phone
        ));
        assert!(matches!(parse_field_type("tel").unwrap(), FieldType::Phone));
        assert!(matches!(
            parse_field_type("telephone").unwrap(),
            FieldType::Phone
        ));
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
            ("social", FieldType::Social),
            ("twitter", FieldType::Social),
            ("instagram", FieldType::Social),
            ("linkedin", FieldType::Social),
            ("custom", FieldType::Custom),
            ("other", FieldType::Custom),
            ("note", FieldType::Custom),
        ]
    }

    proptest! {
        /// Case-insensitive: any mixed-case variant of a valid alias is accepted.
        #[test]
        fn prop_parse_field_type_case_insensitive(
            idx in 0usize..18,
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
                std::mem::discriminant(&result.unwrap()),
                std::mem::discriminant(expected_type),
            );
        }

        /// Arbitrary non-alias strings always produce an error.
        #[test]
        fn prop_parse_field_type_rejects_unknown(s in "\\PC{0,100}") {
            let known: Vec<&str> = vec![
                "email", "mail", "phone", "tel", "telephone",
                "website", "web", "url", "address", "addr", "home",
                "social", "twitter", "instagram", "linkedin",
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
