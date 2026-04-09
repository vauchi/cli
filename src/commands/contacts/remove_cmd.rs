// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;

use crate::commands::common::{drain_activity_log, open_vauchi, register_activity_log_handler};
use crate::config::CliConfig;
use crate::display;

/// Removes a contact.
pub fn remove(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;
    let event_rx = register_activity_log_handler(&wb);

    // Get contact name before removing
    let contact = wb.get_contact(id)?;
    let name = contact.as_ref().map(|c| c.display_name().to_string());

    if wb.remove_contact(id)? {
        display::success(&format!(
            "Removed contact: {}",
            name.unwrap_or_else(|| id.to_string())
        ));
    } else {
        display::warning(&format!("Contact '{}' not found", id));
    }

    drain_activity_log(&wb, event_rx);

    Ok(())
}
