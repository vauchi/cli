// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Activity Command
//!
//! View recent activity and notifications from the persistent log.

use anyhow::Result;

use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

/// Runs the activity command.
pub fn run(config: &CliConfig, since_mins: u64) -> Result<()> {
    let wb = open_vauchi(config)?;

    // Read window over persisted activity rows — injectable CLI clock so
    // E2E scenarios filter against the same timeline the rows were
    // written with.
    let now = crate::clock::unix_seconds();

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
