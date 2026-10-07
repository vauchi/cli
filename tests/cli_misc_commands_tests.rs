// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Card-add argument routing, the pending-QR guard, join's
//! already-initialized warning and the wizards and support page, through
//! the binary (vauchi/private#505).

mod common;

use std::io::Write as _;
use std::process::{Command, Stdio};

use common::Cli;

fn initialized() -> Cli {
    let cli = Cli::new();
    cli.stdout(&["init", "Alice"]);
    cli
}

fn stderr_of(cli: &Cli, args: &[&str]) -> String {
    let output = cli.run(args);
    assert!(!output.status.success(), "{args:?} should fail");
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// Runs a command with stdin closed at once, in its own data directory so
/// nothing lands in the checkout.
fn with_empty_stdin(args: &[&str]) -> String {
    let dir = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_vauchi"))
        .current_dir(dir.path())
        .arg("--data-dir")
        .arg(dir.path().join("data"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn vauchi");
    child.stdin.take().expect("stdin").write_all(b"").unwrap();
    let output = child.wait_with_output().expect("await vauchi");
    String::from_utf8_lossy(&output.stdout).to_string()
}

// @internal
#[test]
fn card_add_without_label_and_value_routes_social_types_to_the_picker() {
    let cli = initialized();

    let plain = stderr_of(&cli, &["card", "add", "email"]);
    assert!(plain.contains("Missing required arguments"), "{plain}");

    let social = cli.run(&["card", "add", "github"]);
    let social_out = format!(
        "{}{}",
        String::from_utf8_lossy(&social.stdout),
        String::from_utf8_lossy(&social.stderr)
    );
    assert!(
        !social_out.contains("Missing required arguments"),
        "a social type goes to the interactive picker: {social_out}"
    );
}

// @internal
#[test]
fn a_corrupt_pending_qr_is_refused_before_it_is_read() {
    let cli = initialized();
    cli.stdout(&["exchange", "start"]);
    let pending = cli.data_dir().join(".pending_qr_exchange");
    let mut bytes = std::fs::read(&pending).unwrap();
    bytes[0] ^= 0xff;
    std::fs::write(&pending, bytes).unwrap();

    let peer = initialized();
    let peer_qr = common::exchange_data(&peer.stdout(&["exchange", "start"]));

    let stderr = stderr_of(&cli, &["exchange", "complete", &peer_qr]);

    assert!(
        stderr.contains("Pending QR exchange state is invalid"),
        "{stderr}"
    );
}

// @internal
#[test]
fn join_on_an_initialized_device_warns_before_replacing_it() {
    let cli = initialized();

    let output = cli.run(&["device", "join", "not-a-link-qr", "--yes"]);

    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("Vauchi is already initialized on this device."),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

// @internal
#[test]
fn the_wizards_render_their_first_surface() {
    for args in [
        &["onboarding"][..],
        &["device", "replace", "setup"][..],
        &["device", "replace", "transfer"][..],
        &["device", "replace", "post-restore"][..],
    ] {
        let output = with_empty_stdin(args);
        assert!(
            output.lines().any(|line| !line.trim().is_empty()),
            "{args:?}"
        );
    }
}

// @internal
#[test]
fn the_support_page_prints_its_title() {
    let output = Cli::new().stdout(&["support-us"]);

    assert!(output.lines().count() > 5, "{output}");
}

// The last-resort error line names the failure in the user's language: it
// read a hardcoded "Error:" (vauchi/private#543).
// @internal
#[test]
fn the_error_line_is_named_in_the_chosen_locale() {
    let cli = initialized();

    let german = stderr_of(&cli, &["--locale", "de", "card", "add", "email"]);

    assert!(german.contains("Fehler:"), "{german}");
    assert!(!german.contains("Error:"), "{german}");
}
