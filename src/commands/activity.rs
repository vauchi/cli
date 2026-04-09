// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Activity Command
//!
//! View recent activity and notifications from the persistent log.

use anyhow::Result;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

/// Runs the activity command.
pub fn run(config: &CliConfig, since_mins: u64) -> Result<()> {
    let wb = open_vauchi(config)?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();

    let since_secs = since_mins * 60;

    let rows = wb.activity_log_poll(now.saturating_sub(since_secs), now)?;

    if rows.is_empty() {
        display::info(&format!("No activity in the last {} minutes.", since_mins));
        return Ok(());
    }

    println!();
    println!("{}", console::style("Recent Activity").bold().underlined());
    println!();

    for row in rows {
        display::display_activity_row(&row);
    }

    Ok(())
}
