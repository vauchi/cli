// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{Result, bail};

use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

/// Marks a contact as trusted for recovery.
pub fn trust(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let mut contact = wb
        .get_contact(id)?
        .or_else(|| {
            wb.search_contacts(id)
                .ok()
                .and_then(|results| results.into_iter().next())
        })
        .ok_or_else(|| anyhow::anyhow!("Contact '{}' not found", id))?;

    let name = contact.display_name().to_string();

    // Blocked contacts cannot be trusted for recovery
    if contact.is_blocked() {
        bail!("Blocked contacts cannot be trusted for recovery");
    }

    if contact.is_recovery_trusted() {
        display::info(&format!("{} is already trusted for recovery", name));
        return Ok(());
    }

    contact.trust_for_recovery()?;
    wb.update_contact(&contact)?;
    display::success(&format!("Marked {} as trusted for recovery", name));

    Ok(())
}

/// Removes recovery trust from a contact.
pub fn untrust(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    let mut contact = wb
        .get_contact(id)?
        .or_else(|| {
            wb.search_contacts(id)
                .ok()
                .and_then(|results| results.into_iter().next())
        })
        .ok_or_else(|| anyhow::anyhow!("Contact '{}' not found", id))?;

    let name = contact.display_name().to_string();

    if !contact.is_recovery_trusted() {
        display::info(&format!("{} is not recovery-trusted", name));
        return Ok(());
    }

    contact.untrust_for_recovery()?;
    wb.update_contact(&contact)?;
    display::success(&format!("Removed recovery trust from {}", name));

    // Check if trusted count drops below threshold
    let readiness = wb.get_recovery_readiness()?;
    if !readiness.is_ready {
        display::warning(&format!(
            "Only {} trusted contact(s) remaining (recovery needs {})",
            readiness.trusted_count, readiness.threshold
        ));
    }

    Ok(())
}
