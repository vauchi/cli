// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::io::Write;

use anyhow::Result;
use vauchi_core::contact_card::vcard::export_vcard;

use super::find_contact;
use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

/// Exports a contact as vCard (.vcf format).
pub fn export(config: &CliConfig, id_or_name: &str, output_path: &str) -> Result<()> {
    let wb = open_vauchi(config)?;

    // Find contact by ID or name
    let contact = find_contact(&wb, id_or_name)?;
    let contact_name = contact.display_name().to_string();

    // Generate vCard from contact's card
    let vcard_content = export_vcard(contact.card());

    // Write to file
    let mut file = File::create(output_path)?;
    file.write_all(vcard_content.as_bytes())?;

    display::success(&format!("Exported {} to {}", contact_name, output_path));

    Ok(())
}
