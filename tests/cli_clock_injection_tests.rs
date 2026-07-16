// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the `VAUCHI_TEST_CLOCK_EPOCH` seam: persisted
//! CLI state must follow the injected clock.

use std::process::{Command, Output};
use tempfile::TempDir;

/// Spawn the CLI against `data_dir` with extra environment, returning raw output.
fn vauchi_env(data_dir: &TempDir, env: &[(&str, &str)], args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_vauchi"));
    cmd.arg("--data-dir").arg(data_dir.path());
    cmd.arg("--relay").arg("ws://127.0.0.1:8080");
    for (key, value) in env {
        cmd.env(key, value);
    }
    cmd.args(args);
    cmd.output().expect("Failed to execute command")
}

/// Assert the command succeeds and return stdout.
fn vauchi_ok(data_dir: &TempDir, env: &[(&str, &str)], args: &[&str]) -> String {
    let output = vauchi_env(data_dir, env, args);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        output.status.success(),
        "Command {args:?} failed.\nStdout: {stdout}\nStderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout
}

/// The E2E harness runs each CLI command as a separate process and pins
/// `VAUCHI_TEST_CLOCK_EPOCH` per invocation (clock-skew / longitudinal
/// scenarios). Persisted timestamps — here an activity-log row's
/// `created_at`, stamped by `drain_activity_log` — must reflect the
/// injected epoch, and the `activity` read window must filter against it.
// @internal
#[test]
fn activity_log_uses_injected_clock() {
    let data_dir = TempDir::new().expect("temp dir");
    vauchi_ok(&data_dir, &[], &["init", "Alice"]);

    // 1_700_000_000 = 2023-11-14T22:13:20Z.
    vauchi_ok(
        &data_dir,
        &[("VAUCHI_TEST_CLOCK_EPOCH", "1700000000")],
        &["card", "add", "email", "work", "alice@example.com"],
    );

    // Read the log with the clock pinned 100s after the write so the
    // row falls inside the `--since 60` window.
    let output = vauchi_ok(
        &data_dir,
        &[("VAUCHI_TEST_CLOCK_EPOCH", "1700000100")],
        &["activity", "--since", "60"],
    );
    assert!(
        output.contains("2023-11-14 22:13:20"),
        "activity should display the injected timestamp, got: {output}"
    );

    // Control: with the real clock the 2023 row is far outside the
    // 60-minute window, so nothing is shown.
    let output = vauchi_ok(&data_dir, &[], &["activity", "--since", "60"]);
    assert!(
        output.contains("No activity in the last 60 minutes"),
        "activity without injected clock should hide the 2023 row, got: {output}"
    );
}
