// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{Result, bail};

use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

/// Shows or sets the contact limit.
///
/// Without `--set`, shows the current contact limit and usage.
/// With `--set N`, updates the maximum number of contacts allowed.
///
/// # Examples
///
/// ```text
/// vauchi contacts limit
/// vauchi contacts limit --set 500
/// ```
pub fn limit(config: &CliConfig, set_value: Option<usize>) -> Result<()> {
    let wb = open_vauchi(config)?;

    match set_value {
        Some(new_limit) => {
            // Validate the new limit
            if new_limit == 0 {
                bail!("Contact limit must be at least 1");
            }

            // Check if current count exceeds new limit
            let current_count = wb.contact_count().unwrap_or(0);
            if current_count > new_limit {
                display::warning(&format!(
                    "You have {} contacts, which exceeds the new limit of {}.",
                    current_count, new_limit
                ));
                display::info(
                    "Existing contacts will not be removed, but no new contacts can be added.",
                );
            }

            wb.storage().set_contact_limit(new_limit)?;
            display::success(&format!("Contact limit set to {}", new_limit));
        }
        None => {
            let max_contacts = wb.storage().get_contact_limit()?;
            let current_count = wb.contact_count().unwrap_or(0);

            println!();
            println!("Contact limit: {} / {}", current_count, max_contacts);

            if current_count >= max_contacts {
                display::warning("Contact limit reached. Remove contacts or increase the limit.");
            } else {
                let remaining = max_contacts - current_count;
                display::info(&format!("{} contact slots remaining.", remaining));
            }
            println!();
        }
    }

    Ok(())
}
