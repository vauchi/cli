// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;

use super::find_contact;
use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

/// Blocks a contact (stops updates in both directions).
pub fn block(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let contact = find_contact(&wb, id)?;
    let name = contact.display_name().to_string();

    if contact.is_blocked() {
        display::info(&format!("{} is already blocked", name));
        return Ok(());
    }

    wb.block_contact(contact.id())?;
    display::success(&format!("Blocked {}", name));
    display::info("They will no longer receive your updates or send you updates.");

    Ok(())
}

/// Unblocks a previously blocked contact.
pub fn unblock(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let contact = find_contact(&wb, id)?;
    let name = contact.display_name().to_string();

    if !contact.is_blocked() {
        display::info(&format!("{} is not blocked", name));
        return Ok(());
    }

    wb.unblock_contact(contact.id())?;
    display::success(&format!("Unblocked {}", name));

    Ok(())
}

/// Lists all blocked contacts.
pub fn list_blocked(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;

    let blocked = wb.list_blocked_contacts()?;

    if blocked.is_empty() {
        display::info("No blocked contacts.");
        return Ok(());
    }

    println!();
    println!("Blocked contacts ({}):", blocked.len());
    println!();

    display::display_contacts_table(&blocked);

    println!();
    display::info("Use 'vauchi contacts unblock <id>' to unblock.");
    println!();

    Ok(())
}
