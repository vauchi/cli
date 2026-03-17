// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;

use super::find_contact;
use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

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
