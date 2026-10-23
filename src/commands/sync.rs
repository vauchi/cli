// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Sync Command
//!
//! Synchronize with the relay server using the core OHTTP HTTP sync API.

use std::time::Duration;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use vauchi_core::api::{FetchPath, VauchiSyncOutcome};

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

    // Core sends fetches through the delayed path once the outer relay has
    // advertised a window, and the answers live in this process only; it
    // must collect them before exiting (private#628). Without a window this
    // is a plain sync.
    let wait = config.delayed_wait;
    let outcome = wb
        .sync_collecting_delayed(wait.poll, wait.max_wait)
        .map_err(|e| anyhow::anyhow!("Sync failed: {e}"))?;

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
            if let Some(warning) = fetch_path_warning(wb.fetch_path()) {
                display::warning(&warning);
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

/// The warning when this sync fetched synchronously because the outer
/// relay withdrew its delayed path (private#628, owner decision
/// 2026-10-23: the fallback is shown, not taken silently).
fn fetch_path_warning(path: FetchPath) -> Option<String> {
    let FetchPath::SynchronousSinceWithdrawn { since_unix } = path else {
        return None;
    };
    let since = chrono::DateTime::from_timestamp(i64::try_from(since_unix).ok()?, 0)?;
    Some(format!(
        "Fetched without delay: the outer relay stopped offering delayed fetching on {}",
        since.format("%Y-%m-%d %H:%M UTC")
    ))
}

// INLINE_TEST_REQUIRED: Binary crate without lib.rs - tests cannot be external
#[cfg(test)]
mod tests {
    use std::time::Duration;

    use vauchi_core::api::FetchPath;

    use super::{fetch_path_warning, sync_summary_line};

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

    // @internal
    #[test]
    fn a_withdrawn_delayed_path_is_warned_about_with_its_time() {
        let warning = fetch_path_warning(FetchPath::SynchronousSinceWithdrawn {
            since_unix: 1_792_754_666,
        });

        assert_eq!(
            warning.as_deref(),
            Some(
                "Fetched without delay: the outer relay stopped offering delayed \
                 fetching on 2026-10-23 11:24 UTC"
            )
        );
    }

    // @internal
    #[test]
    fn a_delayed_or_never_offered_path_gives_no_warning() {
        assert_eq!(
            fetch_path_warning(FetchPath::Delayed {
                window: Duration::from_secs(900)
            }),
            None
        );
        assert_eq!(fetch_path_warning(FetchPath::Synchronous), None);
    }
}
