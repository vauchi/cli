// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use anyhow::{Context, Result};

use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

/// Imports contacts from a vCard (.vcf) file.
pub fn import(config: &CliConfig, file: &Path) -> Result<()> {
    let data = std::fs::read(file).with_context(|| format!("Failed to read {:?}", file))?;
    let wb = open_vauchi(config)?;
    let result = wb.import_contacts_from_vcf(&data)?;

    display::success(&format!("Imported {} contacts", result.imported));
    if result.skipped > 0 {
        display::warning(&format!("Skipped {} contacts", result.skipped));
        for w in &result.warnings {
            eprintln!("  {}", w);
        }
    }

    Ok(())
}
