// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Delivery management commands.
//!
//! Provides CLI access to delivery status, retry processing, cleanup,
//! and human-readable error translation.

use anyhow::Result;

use crate::config::CliConfig;

/// Shows overall delivery status: record counts by status, pending retries, queue state.
pub fn status(_config: &CliConfig) -> Result<()> {
    todo!("Implement delivery status command")
}

/// Lists delivery records, optionally filtered by status.
pub fn list(_config: &CliConfig, _filter: Option<&str>) -> Result<()> {
    todo!("Implement delivery list command")
}

/// Runs the retry scheduler tick, processing due retries.
pub fn retry(_config: &CliConfig) -> Result<()> {
    todo!("Implement delivery retry command")
}

/// Runs delivery cleanup: expires old records, removes terminal records.
pub fn cleanup(_config: &CliConfig) -> Result<()> {
    todo!("Implement delivery cleanup command")
}

/// Translates a failure reason code to a user-friendly message.
pub fn translate(_reason: &str) -> Result<()> {
    todo!("Implement delivery translate command")
}

/// Formats a DeliveryStatus for display.
fn format_delivery_status(_status: &vauchi_core::storage::DeliveryStatus) -> String {
    todo!("Implement format_delivery_status")
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
        // Should succeed with empty delivery records
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
    #[test]
    fn test_connectivity_diagnostics_returns_report() {
        let diagnostics = vauchi_core::delivery::ConnectivityDiagnostics::new();
        let report = diagnostics.run().expect("Diagnostics should succeed");
        assert_eq!(report.offline_queue_capacity, 100);
        assert_eq!(report.offline_queue_depth, 0);
        assert_eq!(report.pending_retries, 0);
    }
}
