// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Sync Command
//!
//! Synchronize with the relay server using the core OHTTP HTTP sync API.

use std::fs;
use std::time::Duration;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use vauchi_core::api::VauchiSyncOutcome;
use vauchi_core::types::{AhaMomentTracker, AhaMomentType};

use crate::commands::common::open_vauchi;
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

    // Connect: bootstrap OHTTP key, health check
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

    spinner.finish_and_clear();
    display::success("Connected");

    // Sync: receive + send
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

    // Display outcome + aha moments
    match outcome {
        VauchiSyncOutcome::Ok {
            received,
            sent,
            acknowledged,
            errors,
        } => {
            println!();
            let total = received + sent + acknowledged;
            if total > 0 {
                let mut summary = format!("Sync complete: {received} received");
                if sent > 0 {
                    summary.push_str(&format!(", {sent} sent"));
                }
                if acknowledged > 0 {
                    summary.push_str(&format!(", {acknowledged} acknowledged"));
                }
                display::success(&summary);
            } else {
                display::info("Sync complete: No new messages or pending updates");
            }
            for err in &errors {
                display::warning(&format!("Sync error: {err}"));
            }

            // Aha moments
            let mut tracker = load_aha_tracker(config);
            if received > 0
                && let Some(moment) = tracker.try_trigger(AhaMomentType::FirstUpdateReceived)
            {
                display::display_aha_moment(&moment);
            }
            if sent > 0
                && let Some(moment) = tracker.try_trigger(AhaMomentType::FirstOutboundDelivered)
            {
                display::display_aha_moment(&moment);
            }
            save_aha_tracker(config, &tracker);
        }
        VauchiSyncOutcome::TooSoon => {
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

/// Load the aha moment tracker from the data directory.
fn load_aha_tracker(config: &CliConfig) -> AhaMomentTracker {
    let path = config.data_dir.join("aha_tracker.json");
    fs::read_to_string(&path)
        .ok()
        .and_then(|json| AhaMomentTracker::from_json(&json).ok())
        .unwrap_or_default()
}

/// Save the aha moment tracker to the data directory.
fn save_aha_tracker(config: &CliConfig, tracker: &AhaMomentTracker) {
    let path = config.data_dir.join("aha_tracker.json");
    if let Ok(json) = tracker.to_json() {
        let _ = crate::config::write_restricted(&path, json);
    }
}
