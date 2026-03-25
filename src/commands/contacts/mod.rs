// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Contacts Command
//!
//! List, view, and manage contacts.

mod block_cmd;
mod export_cmd;
mod favorite_cmd;
mod hide_cmd;
mod import_cmd;
mod limit_cmd;
mod list_cmd;
mod merge_cmd;
mod notes_cmd;
mod open_cmd;
mod remove_cmd;
mod show_cmd;
mod trust_cmd;
mod validation_cmd;
mod verify_cmd;
mod visibility_cmd;

pub use block_cmd::{block, list_blocked, unblock};
pub use export_cmd::export;
pub use favorite_cmd::{favorite, unfavorite};
pub use hide_cmd::{hide_contact, list_hidden, unhide_contact};
pub use import_cmd::import as import_vcf;
pub use limit_cmd::limit;
pub use list_cmd::{list, search};
pub use merge_cmd::{dismiss_duplicate, duplicates, merge, undismiss_duplicate};
pub use notes_cmd::{add_note, delete_note, edit_note, show_note};
pub use open_cmd::{open_field, open_interactive};
pub use remove_cmd::remove;
pub use show_cmd::{show, show_validation_status, show_visibility};
pub use trust_cmd::{trust, untrust};
pub use validation_cmd::{revoke_validation, validate_field};
pub use verify_cmd::verify;
pub use visibility_cmd::{hide_field, unhide_field};

use anyhow::{Result, bail};
use vauchi_core::Vauchi;
use vauchi_core::contact_card::ContactAction;

/// Helper to find contact by ID or name
fn find_contact(wb: &Vauchi, id_or_name: &str) -> Result<vauchi_core::Contact> {
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
fn find_field_id(wb: &Vauchi, label: &str) -> Result<String> {
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

/// Truncates a string for display. Safe for multi-byte UTF-8.
fn truncate_value(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        match s.char_indices().nth(max) {
            Some((idx, _)) => &s[..idx],
            None => s,
        }
    }
}

/// Executes a ContactAction by opening the appropriate URI.
fn execute_action(action: &ContactAction) -> Result<()> {
    use crate::display;

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

// ===========================================================================
// Tests
// ===========================================================================

// INLINE_TEST_REQUIRED: Binary crate without lib.rs — tests cannot be external
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_truncate_value_ascii_within_limit() {
        assert_eq!(truncate_value("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_value_ascii_exact_limit() {
        assert_eq!(truncate_value("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_value_ascii_over_limit() {
        assert_eq!(truncate_value("hello world", 5), "hello");
    }

    #[test]
    fn test_truncate_value_empty() {
        assert_eq!(truncate_value("", 5), "");
    }

    #[test]
    fn test_truncate_value_zero_max() {
        assert_eq!(truncate_value("hello", 0), "");
    }

    #[test]
    fn test_truncate_value_multibyte_emoji() {
        let emoji = "👋hello";
        let result = truncate_value(emoji, 1);
        assert_eq!(result, "👋");
    }

    #[test]
    fn test_truncate_value_cjk() {
        let cjk = "你好世界";
        assert_eq!(truncate_value(cjk, 2), "你好");
    }

    #[test]
    fn test_truncate_value_mixed_ascii_emoji() {
        let mixed = "hi 👋 there";
        let result = truncate_value(mixed, 4);
        assert_eq!(result, "hi 👋");
    }

    #[test]
    fn test_truncate_value_combining_chars() {
        let combining = "e\u{0301}llo";
        let result = truncate_value(combining, 2);
        assert_eq!(result, "e\u{0301}");
    }

    // CC-04: Property-based tests for adversarial Unicode
    proptest! {
        #[test]
        fn prop_truncate_never_panics(
            s in "\\PC{0,200}",
            max in 0usize..100,
        ) {
            // allow(zero_assertions): No-panic fuzz test
            let _ = truncate_value(&s, max);
        }

        #[test]
        fn prop_truncate_respects_max_chars(
            s in "\\PC{0,200}",
            max in 0usize..100,
        ) {
            let result = truncate_value(&s, max);
            prop_assert!(
                result.chars().count() <= max,
                "Result '{}' has {} chars, expected <= {}",
                result, result.chars().count(), max
            );
        }

        #[test]
        fn prop_truncate_valid_utf8(
            s in "\\PC{0,200}",
            max in 0usize..100,
        ) {
            let result = truncate_value(&s, max);
            prop_assert!(result.is_char_boundary(result.len()));
        }

        #[test]
        fn prop_truncate_is_prefix(
            s in "\\PC{0,200}",
            max in 0usize..100,
        ) {
            let result = truncate_value(&s, max);
            prop_assert!(s.starts_with(result));
        }

        #[test]
        fn prop_truncate_noop_when_short(
            s in "\\PC{0,50}",
        ) {
            let char_count = s.chars().count();
            let result = truncate_value(&s, char_count + 10);
            prop_assert_eq!(result, s.as_str());
        }
    }
}
