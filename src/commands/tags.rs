// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Contact Tag Commands
//!
//! Manage the owner-private tag vocabulary (ADR-051). Tags sync to the
//! owner's linked devices but are never shared with contacts.

use anyhow::{Result, anyhow};
use vauchi_core::Vauchi;
use vauchi_core::contact::Tag;

use crate::commands::common::open_vauchi;
use crate::commands::contacts::find_contact;
use crate::config::CliConfig;
use crate::display;

/// Helper to find a tag by (case-insensitive) name.
fn find_tag(wb: &Vauchi, name: &str) -> Result<Tag> {
    wb.find_tag_by_name(name)?
        .ok_or_else(|| anyhow!("Tag not found: {}", name))
}

/// List all tags with their member contacts.
pub fn list(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;
    let mut tags = wb.list_tags()?;

    if tags.is_empty() {
        display::info("No tags defined. Create one with 'vauchi tags create <name>'");
        return Ok(());
    }

    tags.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    for tag in tags {
        println!(
            "  {} ({})",
            tag.name,
            tag.id.chars().take(8).collect::<String>()
        );
        println!("    Contacts: {}", tag.contact_ids.len());
        let mut members: Vec<&String> = tag.contact_ids.iter().collect();
        members.sort();
        for member in members {
            println!("    - {}", member);
        }
    }

    Ok(())
}

/// Create a new tag.
pub fn create(config: &CliConfig, name: &str) -> Result<()> {
    let wb = open_vauchi(config)?;
    let tag = wb.create_tag(name)?;

    display::success(&format!("Created tag '{}'", tag.name));
    Ok(())
}

/// Delete a tag.
pub fn delete(config: &CliConfig, name: &str) -> Result<()> {
    let wb = open_vauchi(config)?;
    let tag = find_tag(&wb, name)?;
    wb.delete_tag(&tag.id)?;

    display::success(&format!("Deleted tag '{}'", tag.name));
    Ok(())
}

/// Add a tag to a contact, creating the tag if it does not exist yet.
pub fn add_contact(config: &CliConfig, tag_name: &str, contact_id_or_name: &str) -> Result<()> {
    let wb = open_vauchi(config)?;
    let contact = find_contact(&wb, contact_id_or_name)?;
    let tag = wb.add_tag_to_contact(contact.id(), tag_name)?;

    display::success(&format!(
        "Added tag '{}' to {}",
        tag.name,
        contact.display_name()
    ));
    Ok(())
}

/// Remove a tag from a contact.
pub fn remove_contact(config: &CliConfig, tag_name: &str, contact_id_or_name: &str) -> Result<()> {
    let wb = open_vauchi(config)?;
    let contact = find_contact(&wb, contact_id_or_name)?;
    let tag = find_tag(&wb, tag_name)?;
    wb.remove_tag_from_contact(&tag.id, contact.id())?;

    display::success(&format!(
        "Removed tag '{}' from {}",
        tag.name,
        contact.display_name()
    ));
    Ok(())
}
