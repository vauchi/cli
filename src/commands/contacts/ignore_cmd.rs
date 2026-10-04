// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;

use super::find_contact;
use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

pub fn ignore(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;
    let contact = find_contact(&wb, id)?;
    wb.ignore_contact(contact.id())?;
    display::success(&format!("Ignored contact: {}", contact.display_name()));
    Ok(())
}

pub fn unignore(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;
    let contact = find_contact(&wb, id)?;
    wb.unignore_contact(contact.id())?;
    display::success(&format!("Unignored contact: {}", contact.display_name()));
    Ok(())
}
