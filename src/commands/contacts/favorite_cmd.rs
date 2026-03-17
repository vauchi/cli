// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;

use super::find_contact;
use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

/// Marks a contact as a favorite.
pub fn favorite(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let mut contact = find_contact(&wb, id)?;
    let name = contact.display_name().to_string();

    if contact.is_favorite() {
        display::info(&format!("{} is already a favorite", name));
        return Ok(());
    }

    contact.set_favorite(true);
    wb.update_contact(&contact)?;
    display::success(&format!("Marked {} as a favorite", name));

    Ok(())
}

/// Removes a contact from favorites.
pub fn unfavorite(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let mut contact = find_contact(&wb, id)?;
    let name = contact.display_name().to_string();

    if !contact.is_favorite() {
        display::info(&format!("{} is not a favorite", name));
        return Ok(());
    }

    contact.set_favorite(false);
    wb.update_contact(&contact)?;
    display::success(&format!("Removed {} from favorites", name));

    Ok(())
}
