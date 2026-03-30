// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{Result, bail};

use super::find_contact;
use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

pub fn archive(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;
    let contact = find_contact(&wb, id)?;

    if !contact.is_exchanged() {
        bail!("Only exchanged contacts can be archived. Use 'delete' for imported contacts.");
    }

    let name = contact.display_name().to_string();
    wb.archive_contact(contact.id())?;
    display::success(&format!("Archived contact: {}", name));
    Ok(())
}

pub fn unarchive(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;
    let contact = find_contact(&wb, id)?;
    let name = contact.display_name().to_string();
    wb.unarchive_contact(contact.id())?;
    display::success(&format!("Unarchived contact: {}", name));
    Ok(())
}

pub fn list_archived(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;
    let archived = wb.list_archived_contacts()?;

    if archived.is_empty() {
        display::info("No archived contacts.");
        return Ok(());
    }

    println!();
    println!("Archived contacts ({}):", archived.len());
    println!();

    display::display_contacts_table(&archived);

    println!();
    display::info("Use 'vauchi contacts unarchive <id>' to restore.");
    println!();

    Ok(())
}
