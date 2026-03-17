// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{bail, Result};

use super::find_contact;
use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

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
