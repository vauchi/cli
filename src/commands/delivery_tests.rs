// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Delivery listing by status filter, the retry report and each status's
//! text (vauchi/private#505).

use tempfile::TempDir;
use vauchi_core::network::RetryTickResult;
use vauchi_core::storage::{DeliveryRecord, DeliveryStatus};

use super::{format_delivery_status, list_lines, retry_lines};
use crate::commands::common::open_vauchi;
use crate::config::CliConfig;

fn plain(lines: Vec<String>) -> Vec<String> {
    lines
        .iter()
        .map(|line| console::strip_ansi_codes(line).to_string())
        .collect()
}

fn record(id: &str, status: DeliveryStatus) -> DeliveryRecord {
    DeliveryRecord {
        message_id: id.into(),
        recipient_id: "bob".into(),
        status,
        // Recent: Storage::open deletes terminal records older than 30 days.
        created_at: crate::clock::unix_seconds(),
        updated_at: crate::clock::unix_seconds(),
        expires_at: None,
    }
}

fn config_with_records() -> (TempDir, CliConfig) {
    let dir = TempDir::new().unwrap();
    let config = CliConfig {
        data_dir: dir.path().to_path_buf(),
        relay_url: "ws://127.0.0.1:9".to_string(),
        ohttp_relay_url: None,
        raw: false,
    };
    crate::commands::init::run("Alice", false, &config, "en").unwrap();
    let wb = open_vauchi(&config).unwrap();
    for (id, status) in [
        ("queued-0001", DeliveryStatus::Queued),
        (
            "failed-0001",
            DeliveryStatus::Failed {
                reason: "timeout".into(),
            },
        ),
        ("deliver-001", DeliveryStatus::Delivered),
    ] {
        wb.storage()
            .deliveries()
            .create_delivery_record(&record(id, status))
            .unwrap();
    }
    (dir, config)
}

// @internal
#[test]
fn the_listing_follows_the_status_filter() {
    let (_dir, config) = config_with_records();
    let wb = open_vauchi(&config).unwrap();
    let ids = |filter| -> Vec<String> {
        plain(list_lines(wb.storage(), filter).unwrap())
            .into_iter()
            .filter_map(|line| line.trim().split(' ').next().map(str::to_string))
            .filter(|id| id.len() == 8)
            .collect()
    };

    let mut all = ids(None);
    all.sort();
    assert_eq!(all, ["deliver-", "failed-0", "queued-0"]);
    assert_eq!(ids(Some("failed")), ["failed-0"]);
    assert_eq!(ids(Some("pending")), ["queued-0"]);
}

// @internal
#[test]
fn an_empty_listing_says_so_and_a_full_one_counts_its_records() {
    let (_dir, config) = config_with_records();
    let wb = open_vauchi(&config).unwrap();

    let all = plain(list_lines(wb.storage(), None).unwrap());
    assert_eq!(all[0], "ℹ 3 delivery record(s):");
    assert!(
        all.contains(&"  failed-0 -> bob [failed: timeout]".to_string()),
        "{all:?}"
    );
}

// @internal
#[test]
fn the_retry_report_lists_what_is_ready_to_resend() {
    let idle = RetryTickResult {
        due: 0,
        rescheduled: 0,
        expired: 0,
        ready_ids: Vec::new(),
    };
    let busy = RetryTickResult {
        due: 3,
        rescheduled: 2,
        expired: 1,
        ready_ids: vec!["m1".into(), "m2".into()],
    };
    let nothing_ready = RetryTickResult {
        due: 1,
        rescheduled: 0,
        expired: 1,
        ready_ids: Vec::new(),
    };

    assert_eq!(plain(retry_lines(&idle)), ["ℹ No retries due."]);
    assert_eq!(
        plain(retry_lines(&busy)),
        [
            "✓ Processed 3 due retries: 2 rescheduled, 1 expired",
            "  Ready for resend:",
            "    m1",
            "    m2",
        ]
    );
    assert_eq!(
        plain(retry_lines(&nothing_ready)),
        ["✓ Processed 1 due retries: 0 rescheduled, 1 expired"]
    );
}

// @internal
#[test]
fn each_status_has_its_word() {
    assert_eq!(format_delivery_status(&DeliveryStatus::Sent), "sent");
    assert_eq!(format_delivery_status(&DeliveryStatus::Stored), "stored");
    assert_eq!(format_delivery_status(&DeliveryStatus::Expired), "expired");
}
