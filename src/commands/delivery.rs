// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Delivery management commands.
//!
//! Provides CLI access to delivery status, retry processing, cleanup,
//! and human-readable error translation.

use anyhow::Result;

use crate::config::CliConfig;
use crate::display;

use super::common::open_vauchi;

/// Shows overall delivery status: record counts by status, pending retries, queue state.
pub fn status(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;
    let storage = wb.storage();

    let diagnostics = vauchi_core::delivery::ConnectivityDiagnostics::new();
    let report = diagnostics
        .run()
        .map_err(|e| anyhow::anyhow!("Diagnostics failed: {}", e))?;

    let queued =
        storage.count_deliveries_by_status(&vauchi_core::storage::DeliveryStatus::Queued)?;
    let sent = storage.count_deliveries_by_status(&vauchi_core::storage::DeliveryStatus::Sent)?;
    let stored =
        storage.count_deliveries_by_status(&vauchi_core::storage::DeliveryStatus::Stored)?;
    let delivered =
        storage.count_deliveries_by_status(&vauchi_core::storage::DeliveryStatus::Delivered)?;
    let failed =
        storage.count_deliveries_by_status(&vauchi_core::storage::DeliveryStatus::Failed {
            reason: String::new(),
        })?;

    display::info("Delivery Status");
    println!();
    println!("  Queued:     {}", queued);
    println!("  Sent:       {}", sent);
    println!("  Stored:     {}", stored);
    println!("  Delivered:  {}", delivered);
    println!("  Failed:     {}", failed);
    println!();
    println!("  Pending retries:      {}", report.pending_retries);
    println!("  Offline queue depth:  {}", report.offline_queue_depth);
    println!("  Queue capacity:       {}", report.offline_queue_capacity);

    if !report.next_retry_at.is_empty() {
        println!("  Next retry:           {}", report.next_retry_at);
    }

    Ok(())
}

/// Lists delivery records, optionally filtered by status.
pub fn list(config: &CliConfig, filter: Option<&str>) -> Result<()> {
    let wb = open_vauchi(config)?;
    let storage = wb.storage();

    let records = match filter {
        Some("failed") => storage.get_delivery_records_by_status(
            &vauchi_core::storage::DeliveryStatus::Failed {
                reason: String::new(),
            },
        )?,
        Some("pending") => storage.get_pending_deliveries()?,
        _ => storage.get_all_delivery_records()?,
    };

    if records.is_empty() {
        display::info("No delivery records found.");
        return Ok(());
    }

    display::info(&format!("{} delivery record(s):", records.len()));
    println!();

    for record in &records {
        let status_str = format_delivery_status(&record.status);
        let id_prefix = &record.message_id[..8.min(record.message_id.len())];
        println!(
            "  {} -> {} [{}]",
            id_prefix, record.recipient_id, status_str
        );
    }

    Ok(())
}

/// Runs the retry scheduler tick, processing due retries.
pub fn retry(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;
    let storage = wb.storage();

    let scheduler = vauchi_core::delivery::RetryScheduler::new();
    let result = scheduler.tick(storage)?;

    if result.due == 0 {
        display::info("No retries due.");
    } else {
        display::success(&format!(
            "Processed {} due retries: {} rescheduled, {} expired",
            result.due, result.rescheduled, result.expired
        ));

        if !result.ready_ids.is_empty() {
            println!("  Ready for resend:");
            for id in &result.ready_ids {
                println!("    {}", id);
            }
        }
    }

    Ok(())
}

/// Runs delivery cleanup: expires old records, removes terminal records.
pub fn cleanup(config: &CliConfig) -> Result<()> {
    let wb = open_vauchi(config)?;
    let storage = wb.storage();

    let service = vauchi_core::delivery::DeliveryService::new();
    let result = service.run_cleanup(storage)?;

    display::success(&format!(
        "Cleanup complete: {} expired, {} removed",
        result.expired, result.cleaned_up
    ));

    Ok(())
}

/// Translates a failure reason code to a user-friendly message.
pub fn translate(reason: &str) -> Result<()> {
    let message = vauchi_core::delivery::failure_to_user_message(reason);
    println!("{}", message);
    Ok(())
}

/// Formats a DeliveryStatus for display.
fn format_delivery_status(status: &vauchi_core::storage::DeliveryStatus) -> String {
    match status {
        vauchi_core::storage::DeliveryStatus::Queued => "queued".to_string(),
        vauchi_core::storage::DeliveryStatus::Sent => "sent".to_string(),
        vauchi_core::storage::DeliveryStatus::Stored => "stored".to_string(),
        vauchi_core::storage::DeliveryStatus::Delivered => "delivered".to_string(),
        vauchi_core::storage::DeliveryStatus::Expired => "expired".to_string(),
        vauchi_core::storage::DeliveryStatus::Failed { reason } => {
            format!("failed: {}", reason)
        }
    }
}

// INLINE_TEST_REQUIRED: Binary crate without lib.rs - tests cannot be external
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CliConfig;

    /// Creates a CliConfig with an initialized identity for testing.
    fn setup_test_config() -> (tempfile::TempDir, CliConfig) {
        let dir = tempfile::tempdir().unwrap();
        let config = CliConfig {
            data_dir: dir.path().to_path_buf(),
            relay_url: "wss://test.example.com".to_string(),
        };

        // Initialize identity so open_vauchi works
        let identity = vauchi_core::Identity::create("TestUser");
        config
            .save_local_identity(&identity)
            .expect("save identity");

        (dir, config)
    }

    // @scenario: message_delivery:Delivery status command shows counts
    #[test]
    fn test_status_shows_delivery_counts() {
        let (_dir, config) = setup_test_config();
        let result = status(&config);
        assert!(
            result.is_ok(),
            "Status command should succeed: {:?}",
            result.err()
        );
    }

    // @scenario: message_delivery:Delivery list with no records
    #[test]
    fn test_list_empty_shows_no_records() {
        let (_dir, config) = setup_test_config();
        let result = list(&config, None);
        assert!(
            result.is_ok(),
            "List command should succeed: {:?}",
            result.err()
        );
    }

    // @scenario: message_delivery:Delivery list with filter
    #[test]
    fn test_list_with_failed_filter() {
        let (_dir, config) = setup_test_config();
        let result = list(&config, Some("failed"));
        assert!(
            result.is_ok(),
            "List with filter should succeed: {:?}",
            result.err()
        );
    }

    // @scenario: message_delivery:Retry tick with no due entries
    #[test]
    fn test_retry_with_no_due_entries() {
        let (_dir, config) = setup_test_config();
        let result = retry(&config);
        assert!(
            result.is_ok(),
            "Retry command should succeed: {:?}",
            result.err()
        );
    }

    // @scenario: message_delivery:Cleanup removes old records
    #[test]
    fn test_cleanup_succeeds() {
        let (_dir, config) = setup_test_config();
        let result = cleanup(&config);
        assert!(
            result.is_ok(),
            "Cleanup command should succeed: {:?}",
            result.err()
        );
    }

    // @scenario: message_delivery:Failure reason translated to user message
    #[test]
    fn test_translate_known_reason() {
        let result = translate("connection_timeout");
        assert!(
            result.is_ok(),
            "Translate should succeed: {:?}",
            result.err()
        );
    }

    // @scenario: message_delivery:Delivery status formatting
    #[test]
    fn test_format_delivery_status_queued() {
        let status = vauchi_core::storage::DeliveryStatus::Queued;
        assert_eq!(format_delivery_status(&status), "queued");
    }

    #[test]
    fn test_format_delivery_status_failed_includes_reason() {
        let status = vauchi_core::storage::DeliveryStatus::Failed {
            reason: "connection_timeout".to_string(),
        };
        let formatted = format_delivery_status(&status);
        assert_eq!(formatted, "failed: connection_timeout");
    }

    #[test]
    fn test_format_delivery_status_delivered() {
        let status = vauchi_core::storage::DeliveryStatus::Delivered;
        assert_eq!(format_delivery_status(&status), "delivered");
    }

    // @scenario: message_delivery:ConnectivityDiagnostics report is accessible
    // @scenario: message_delivery.feature:Debug connectivity issues
    #[test]
    fn test_connectivity_diagnostics_returns_report() {
        let diagnostics = vauchi_core::delivery::ConnectivityDiagnostics::new();
        let report = diagnostics.run().expect("Diagnostics should succeed");
        assert_eq!(report.offline_queue_capacity, 100);
        assert_eq!(report.offline_queue_depth, 0);
        assert_eq!(report.pending_retries, 0);
    }
}
