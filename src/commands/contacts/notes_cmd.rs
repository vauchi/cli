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
    use vauchi_core::crypto::encrypt;

    let wb = open_vauchi(config)?;

    // Find contact
    let contact = find_contact(&wb, id_or_name)?;
    let contact_id = contact.id().to_string();
    let contact_name = contact.display_name().to_string();

    // Encrypt note with contact's shared key
    let shared_key = contact
        .shared_key()
        .ok_or_else(|| anyhow::anyhow!("Contact has no shared key (imported contact?)"))?;
    let encrypted = encrypt(shared_key, note_text.as_bytes())?;

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
            let shared_key = contact
                .shared_key()
                .ok_or_else(|| anyhow::anyhow!("Contact has no shared key (imported contact?)"))?;
            let decrypted = decrypt(shared_key, &encrypted)?;
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
    let shared_key = contact
        .shared_key()
        .ok_or_else(|| anyhow::anyhow!("Contact has no shared key (imported contact?)"))?;
    let encrypted = encrypt(shared_key, note_text.as_bytes())?;

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
