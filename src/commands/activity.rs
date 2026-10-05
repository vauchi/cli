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

    let rows = wb.activity_log_poll(window_start(now, since_mins), now)?;

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

/// The earliest timestamp `activity --since <minutes>` shows at `now`.
fn window_start(now: u64, since_mins: u64) -> u64 {
    now.saturating_sub(since_mins * 60)
}

// INLINE_TEST_REQUIRED: Binary crate without lib.rs - tests cannot be external
#[cfg(test)]
mod tests {
    use super::window_start;

    // @internal
    #[test]
    fn the_window_reaches_back_whole_minutes_and_stops_at_zero() {
        assert_eq!(window_start(10_000, 2), 9_880);
        assert_eq!(window_start(10_000, 0), 10_000);
        assert_eq!(window_start(30, 5), 0);
    }
}
