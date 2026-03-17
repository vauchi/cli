// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;

use super::find_contact;
use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

/// Marks a contact's fingerprint as verified.
pub fn verify(config: &CliConfig, id: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    // Find contact by ID or name (supports partial ID prefixes)
    let contact = find_contact(&wb, id)?;
    let contact_id = contact.id().to_string();
    let name = contact.display_name().to_string();

    if contact.is_fingerprint_verified() {
        display::info(&format!("{} is already verified", name));
        return Ok(());
    }

    // Display fingerprints for manual comparison before marking verified
    println!();
    println!("  Their fingerprint ({}):", name);
    println!("  {}", contact.fingerprint());
    if let Ok(own_fp) = wb.own_fingerprint() {
        println!();
        println!("  Your fingerprint:");
        println!("  {}", own_fp);
    }
    println!();
    println!("  Compare these fingerprints in person before verifying.");
    println!();

    wb.verify_contact_fingerprint(&contact_id)?;
    display::success(&format!("Verified fingerprint for {}", name));

    Ok(())
}
