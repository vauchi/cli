// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Sync Command
//!
//! Synchronize with the relay server using the core OHTTP HTTP sync API.

use std::time::Duration;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use vauchi_core::api::VauchiSyncOutcome;

use crate::commands::common::{drain_activity_log, open_vauchi, register_activity_log_handler};
use crate::config::CliConfig;
use crate::display;

/// Runs the sync command.
///
/// Delegates to `Vauchi::connect()` + `sync()` for bidirectional sync
/// over OHTTP-encrypted HTTP. The core API handles:
/// - OHTTP key bootstrap and caching
/// - Mailbox token registration
/// - Blob fetch, ratchet-based decrypt, and ACK
/// - Outbound update encryption and delivery
/// - C1/C2 timing enforcement
pub fn run(config: &CliConfig) -> Result<()> {
    let mut wb = open_vauchi(config)?;

    // Sync is the primary source of background events in the CLI.
    let event_rx = register_activity_log_handler(&wb);

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    spinner.set_message(format!("Connecting to {}...", config.relay_url));
    spinner.enable_steady_tick(Duration::from_millis(80));

    wb.connect()
        .map_err(|e| anyhow::anyhow!("Connection failed: {e}"))?;

    // Real clock on purpose: `start_time` brackets the sync operation so
    // the activity window below spans the sync's actual duration. The
    // injected test clock (VAUCHI_TEST_CLOCK_EPOCH) must not distort
    // elapsed-time measurement.
    let start_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    spinner.finish_and_clear();
    display::success("Connected");

    let sync_spinner = ProgressBar::new_spinner();
    sync_spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.blue} {msg}")
            .unwrap(),
    );
    sync_spinner.set_message("Syncing...");
    sync_spinner.enable_steady_tick(Duration::from_millis(80));

    let outcome = wb.sync().map_err(|e| anyhow::anyhow!("Sync failed: {e}"))?;

    sync_spinner.finish_and_clear();

    drain_activity_log(&wb, event_rx);

    match outcome {
        VauchiSyncOutcome::Ok {
            received,
            sent,
            acknowledged,
            errors,
            ..
        } => {
            println!();
            println!("{}", sync_summary_line(received, sent, acknowledged));
            for err in &errors {
                display::warning(&format!("Sync error: {err}"));
            }

            // Real clock on purpose: pairs with `start_time` above to
            // measure the sync's wall-clock window; the injected test
            // clock must not distort elapsed-time measurement.
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let activity = wb.activity_log_poll(start_time, now)?;
            if !activity.is_empty() {
                println!();
                println!("{}", console::style("Recent Activity").bold().underlined());
                for row in activity {
                    display::display_activity_row(&row);
                }
            }
        }
        VauchiSyncOutcome::TooSoon { .. } => {
            display::info("Sync skipped: too soon since last sync");
        }
        VauchiSyncOutcome::NotConnected => {
            display::warning("Not connected to relay");
        }
        VauchiSyncOutcome::NoIdentity => {
            display::warning("No identity found. Run 'vauchi init <name>' first.");
        }
    }

    wb.disconnect();

    Ok(())
}

/// The line a completed sync reports: the counts that are not zero, or
/// that nothing moved.
fn sync_summary_line(received: usize, sent: usize, acknowledged: usize) -> String {
    if received + sent + acknowledged == 0 {
        return display::info_line("Sync complete: No new messages or pending updates");
    }
    let mut summary = format!("Sync complete: {received} received");
    if sent > 0 {
        summary.push_str(&format!(", {sent} sent"));
    }
    if acknowledged > 0 {
        summary.push_str(&format!(", {acknowledged} acknowledged"));
    }
    display::success_line(&summary)
}

// INLINE_TEST_REQUIRED: Binary crate without lib.rs - tests cannot be external
#[cfg(test)]
mod tests {
    use super::sync_summary_line;

    fn plain(line: String) -> String {
        console::strip_ansi_codes(&line).to_string()
    }

    // @internal
    #[test]
    fn the_summary_names_only_the_counts_that_moved() {
        let cases = [
            (
                (0, 0, 0),
                "ℹ Sync complete: No new messages or pending updates",
            ),
            ((1, 0, 0), "✓ Sync complete: 1 received"),
            ((0, 1, 0), "✓ Sync complete: 0 received, 1 sent"),
            ((0, 0, 1), "✓ Sync complete: 0 received, 1 acknowledged"),
            (
                (2, 3, 4),
                "✓ Sync complete: 2 received, 3 sent, 4 acknowledged",
            ),
        ];
        for ((received, sent, acknowledged), expected) in cases {
            assert_eq!(
                plain(sync_summary_line(received, sent, acknowledged)),
                expected
            );
        }
    }
}
