// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! #387: once an app password is set, every command that opens user data
//! asks for it, and the duress PIN puts every such command in duress mode.
//! Until 2026-10 only a handful of entry points checked it.

mod common;

use common::{Cli, exchange_data};

const PASSWORD: &str = "app-password-123";
const DURESS_PIN: &str = "135790";

/// Answers `duress setup`: the app password twice, then a duress PIN.
const SETUP_ANSWERS: &str = "app-password-123\napp-password-123\n\n1\n135790\n\n135790\n\n\n\n";

fn protect(cli: &Cli) {
    let output = cli.run_with_stdin(&["duress", "setup"], SETUP_ANSWERS);
    assert!(
        output.status.success(),
        "duress setup failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Commands that read or change user data and were open before #387, one
/// or more from each module.
const GATED: &[&[&str]] = &[
    &["card", "show"],
    &["labels", "list"],
    &["tags", "list"],
    &["activity"],
    &["delivery", "status"],
    &["gdpr", "consent-status"],
    &["emergency", "status"],
    &["recovery", "status"],
    &["duress", "status"],
    &["exchange", "start"],
];

// @scenario: duress_pin:Enable duress mode
#[test]
fn test_every_data_command_refuses_without_the_app_password() {
    let cli = Cli::new();
    cli.stdout(&["init", "Alice Smith"]);
    protect(&cli);

    for args in GATED {
        let output = cli.run(args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success() && stderr.contains("App password is configured"),
            "{args:?} ran without the app password: status {:?}, stderr {stderr}",
            output.status.code()
        );
    }
}

// @scenario: duress_pin:Enable duress mode
#[test]
fn test_every_data_command_runs_with_the_app_password() {
    let cli = Cli::new();
    cli.stdout(&["init", "Alice Smith"]);
    protect(&cli);

    for args in GATED {
        let mut with_pin = vec!["--pin", PASSWORD];
        with_pin.extend_from_slice(args);
        let output = cli.run(&with_pin);
        assert!(
            output.status.success(),
            "{args:?} failed with the app password: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// Changing the app password needs the current one.
// @scenario: duress_pin:Enable duress mode
#[test]
fn test_duress_setup_cannot_replace_the_password_without_it() {
    let cli = Cli::new();
    cli.stdout(&["init", "Alice Smith"]);
    protect(&cli);

    let output = cli.run_with_stdin(
        &["duress", "setup"],
        "new-password-456\nnew-password-456\n\n1\n246802\n\n246802\n\n\n\n",
    );

    assert!(!output.status.success(), "the password was replaced");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("App password is configured"),
        "got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Under the duress PIN the personal-data export holds no real contact.
// @scenario: duress_mode :: Cannot access real contacts from duress mode
#[test]
fn test_gdpr_export_under_the_duress_pin_holds_no_real_contact() {
    let alice = Cli::new();
    alice.stdout(&["init", "Alice Smith"]);
    protect(&alice);
    let alice_data = exchange_data(&alice.stdout(&["--pin", PASSWORD, "exchange", "start"]));
    let bob = Cli::new();
    bob.stdout(&["init", "Bob Jones"]);
    let bob_data = exchange_data(&bob.stdout(&["exchange", "start"]));
    bob.stdout(&["exchange", "complete", &alice_data]);
    alice.stdout(&["--pin", PASSWORD, "exchange", "complete", &bob_data]);
    let path = alice.data_dir().join("export.json");
    let path_arg = path.to_str().expect("utf-8 path");

    alice.stdout(&["--pin", DURESS_PIN, "gdpr", "export", path_arg]);

    let export = std::fs::read_to_string(&path).expect("export written");
    assert!(
        !export.contains("Bob Jones"),
        "duress export holds the real contact: {export}"
    );
}
