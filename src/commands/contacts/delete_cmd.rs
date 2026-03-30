// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{Result, bail};

use super::find_contact;
use crate::commands::common::open_vauchi;
use crate::commands::device_sync_helpers::record_contact_removed;
use crate::config::CliConfig;
use crate::display;

pub fn delete(config: &CliConfig, id: &str, yes: bool) -> Result<()> {
    let wb = open_vauchi(config)?;
    let contact = find_contact(&wb, id)?;

    if !contact.is_imported() {
        bail!("Only imported contacts can be deleted. Use 'archive' for exchanged contacts.");
    }

    let name = contact.display_name().to_string();
    let contact_id = contact.id().to_string();

    if yes {
        wb.hard_delete_imported_contact(&contact_id)?;
        display::success(&format!("Deleted contact: {}", name));
        if let Err(e) = record_contact_removed(&wb, &contact_id) {
            display::warning(&format!("Failed to record for device sync: {}", e));
        }
    } else {
        wb.soft_delete_imported_contact(&contact_id)?;
        display::info(&format!(
            "Contact '{}' will be deleted in 30s. Press Ctrl+C to cancel.",
            name
        ));
        std::thread::sleep(std::time::Duration::from_secs(30));
        wb.hard_delete_imported_contact(&contact_id)?;
        display::success(&format!("Deleted contact: {}", name));
        if let Err(e) = record_contact_removed(&wb, &contact_id) {
            display::warning(&format!("Failed to record for device sync: {}", e));
        }
    }

    Ok(())
}
