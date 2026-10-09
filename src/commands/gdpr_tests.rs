// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The GDPR deletion commands past their confirmation prompts
//! (vauchi/private#505). `dialoguer` refuses a non-terminal stdin, so the
//! prompts themselves stay untested; everything after the answer is here.

use tempfile::TempDir;
use vauchi_core::Vauchi;
use vauchi_core::api::{DeletionManager, ShredReport, ShredVerification};
use vauchi_core::storage::DeletionState;

use super::{
    days_and_hours, deletion_status_lines, execute_deletion_answered, panic_shred_answered,
    schedule_deletion_answered, shred_summary_lines, shred_token_after_grace, ws_to_http,
};
use crate::commands::common::open_vauchi;
use crate::config::CliConfig;

const DAY: u64 = 86_400;
const HOUR: u64 = 3_600;

fn initialized() -> (TempDir, CliConfig) {
    let dir = TempDir::new().unwrap();
    let config = CliConfig {
        data_dir: dir.path().to_path_buf(),
        relay_url: "ws://127.0.0.1:9".to_string(),
        ohttp_relay_url: None,
        relay_anchor: None,
        raw: false,
    };
    crate::commands::init::run("Alice", false, &config, "en").unwrap();
    (dir, config)
}

fn deletion_state(wb: &Vauchi) -> DeletionState {
    DeletionManager::new(wb.storage()).deletion_state().unwrap()
}

fn plain(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .map(|line| console::strip_ansi_codes(line).to_string())
        .collect()
}

// @internal
#[test]
fn a_span_splits_into_whole_days_and_remaining_hours() {
    assert_eq!(days_and_hours(7 * DAY), (7, 0));
    assert_eq!(days_and_hours(6 * DAY + 23 * HOUR + 59), (6, 23));
    assert_eq!(days_and_hours(HOUR - 1), (0, 0));
}

// @internal
#[test]
fn scheduling_needs_the_word_delete_in_any_case() {
    let (_dir, config) = initialized();
    let wb = open_vauchi(&config).unwrap();

    assert_eq!(schedule_deletion_answered(&wb, "nope").unwrap(), None);
    assert!(matches!(deletion_state(&wb), DeletionState::None));

    assert_eq!(schedule_deletion_answered(&wb, "DELETE").unwrap(), Some(7));
    assert!(matches!(
        deletion_state(&wb),
        DeletionState::Scheduled { .. }
    ));
}

// @internal
#[test]
fn the_status_names_each_state() {
    let scheduled = DeletionState::Scheduled {
        scheduled_at: 100,
        execute_at: 100 + 7 * DAY,
    };

    assert_eq!(
        plain(&deletion_status_lines(&DeletionState::None, 0)),
        ["ℹ No deletion scheduled."]
    );
    assert_eq!(
        plain(&deletion_status_lines(&scheduled, 100 + HOUR)),
        [
            "⚠ Deletion scheduled at 100 — 6 days, 23 hours remaining.",
            "ℹ Run 'vauchi gdpr cancel-deletion' to cancel.",
        ]
    );
    assert_eq!(
        plain(&deletion_status_lines(
            &DeletionState::Executed { executed_at: 5 },
            0
        )),
        ["⚠ Identity was destroyed at 5."]
    );
}

// @internal
#[test]
fn the_grace_period_gates_the_shred_token() {
    let scheduled = DeletionState::Scheduled {
        scheduled_at: 100,
        execute_at: 100 + 7 * DAY,
    };

    let early = shred_token_after_grace(&scheduled, 100 + 6 * DAY + 2 * HOUR).unwrap_err();
    assert_eq!(
        early.to_string(),
        "Grace period has not elapsed. 0 days, 22 hours remaining."
    );
    assert!(shred_token_after_grace(&scheduled, 100 + 7 * DAY).is_ok());
    assert_eq!(
        shred_token_after_grace(&DeletionState::None, 0)
            .unwrap_err()
            .to_string(),
        "No deletion scheduled. Run 'vauchi gdpr schedule-deletion' first."
    );
    assert_eq!(
        shred_token_after_grace(&DeletionState::Executed { executed_at: 1 }, 0)
            .unwrap_err()
            .to_string(),
        "Identity has already been destroyed."
    );
}

// @internal
#[test]
fn execution_needs_the_exact_word_and_leaves_the_identity_otherwise() {
    let (_dir, config) = initialized();
    let wb = open_vauchi(&config).unwrap();
    schedule_deletion_answered(&wb, "delete").unwrap();
    let scheduled = deletion_state(&wb);
    let token = shred_token_after_grace(&scheduled, u64::MAX).unwrap();

    let declined = execute_deletion_answered(&config, &wb, token, "execute").unwrap();

    assert_eq!(declined, None);
    assert!(wb.identity().is_some());
}

// @internal
#[test]
fn panic_shred_needs_the_exact_word() {
    let (_dir, config) = initialized();
    let wb = open_vauchi(&config).unwrap();

    assert_eq!(panic_shred_answered(&config, &wb, "panic").unwrap(), None);
    assert!(wb.identity().is_some());
}

// @internal
#[test]
fn panic_shred_destroys_the_store_and_reports_it() {
    let (_dir, config) = initialized();
    let wb = open_vauchi(&config).unwrap();

    let lines = plain(
        &panic_shred_answered(&config, &wb, "PANIC")
            .unwrap()
            .unwrap(),
    );

    assert!(
        lines.contains(&"⚠ Executing emergency panic shred...".to_string()),
        "{lines:?}"
    );
    assert!(
        lines.contains(&"  SMK destroyed:          true".to_string()),
        "{lines:?}"
    );
    assert_eq!(
        lines.last().unwrap(),
        "✓ Panic shred complete. All data destroyed."
    );
    assert!(!config.data_dir.join("vauchi.db").exists());
}

// @internal
#[test]
fn the_summary_ends_with_the_verification_verdict() {
    let report = ShredReport::default();
    let mut verification = ShredVerification {
        smk_absent: true,
        bootstrap_key_absent: true,
        database_absent: true,
        data_dir_absent: true,
        pre_signed_absent: true,
        all_clear: true,
    };

    verification.all_clear = true;
    let clear = plain(&shred_summary_lines(&report, &verification));
    verification.all_clear = false;
    let doubtful = plain(&shred_summary_lines(&report, &verification));

    assert_eq!(clear[1], "ℹ === Shred Report ===");
    assert_eq!(
        clear.last().unwrap(),
        "✓   All clear — all data verified destroyed."
    );
    assert_eq!(
        doubtful.last().unwrap(),
        "⚠   WARNING: Some data may not have been fully destroyed."
    );
}

// @internal
#[test]
fn relay_urls_turn_from_websocket_to_http() {
    assert_eq!(ws_to_http("wss://relay.example"), "https://relay.example");
    assert_eq!(ws_to_http("ws://127.0.0.1:8080"), "http://127.0.0.1:8080");
    assert_eq!(ws_to_http("https://relay.example"), "https://relay.example");
}

// @internal
#[test]
fn an_accepted_execution_reaches_cores_own_grace_check() {
    let (_dir, config) = initialized();
    let wb = open_vauchi(&config).unwrap();
    schedule_deletion_answered(&wb, "delete").unwrap();
    let scheduled = deletion_state(&wb);
    // The shell's gate is passed with a far-future clock; Core checks the
    // grace period again with the system clock and refuses, which is the
    // observable sign that the answer was acted on.
    let token = shred_token_after_grace(&scheduled, u64::MAX).unwrap();

    let error = execute_deletion_answered(&config, &wb, token, "EXECUTE").unwrap_err();

    assert_eq!(
        error.to_string(),
        "Shred failed: Deletion error: Grace period not elapsed"
    );
    assert!(wb.identity().is_some());
}
