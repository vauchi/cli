// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;

use super::find_contact;
use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

/// Adds a personal note to a contact.
pub fn add_note(config: &CliConfig, id_or_name: &str, note_text: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let contact = find_contact(&wb, id_or_name)?;
    let contact_id = contact.id().to_string();
    let contact_name = contact.display_name().to_string();

    wb.add_personal_note(&contact_id, note_text)?;

    display::success(&format!("Added note to {}", contact_name));

    Ok(())
}

/// Shows the personal note for a contact.
pub fn show_note(config: &CliConfig, id_or_name: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let contact = find_contact(&wb, id_or_name)?;
    let contact_id = contact.id().to_string();
    let contact_name = contact.display_name().to_string();

    match wb.read_personal_note(&contact_id)? {
        Some(note_text) => {
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
    let wb = open_vauchi(config)?;

    let contact = find_contact(&wb, id_or_name)?;
    let contact_id = contact.id().to_string();
    let contact_name = contact.display_name().to_string();

    wb.add_personal_note(&contact_id, note_text)?;

    display::success(&format!("Updated note for {}", contact_name));

    Ok(())
}

/// Deletes the personal note for a contact.
pub fn delete_note(config: &CliConfig, id_or_name: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let contact = find_contact(&wb, id_or_name)?;
    let contact_id = contact.id().to_string();
    let contact_name = contact.display_name().to_string();

    wb.delete_personal_notes(&contact_id)?;

    display::success(&format!("Deleted note for {}", contact_name));

    Ok(())
}
