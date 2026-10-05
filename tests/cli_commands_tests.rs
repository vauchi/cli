// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Delivery commands on an empty store, and duplicate detection, merging
//! and dismissing through the binary (vauchi/private#505).

mod common;

use common::{Cli, exchange_data};

fn initialized(name: &str) -> Cli {
    let cli = Cli::new();
    cli.stdout(&["init", name]);
    cli
}

fn exchange(a: &Cli, b: &Cli) {
    let a_data = exchange_data(&a.stdout(&["exchange", "start"]));
    let b_data = exchange_data(&b.stdout(&["exchange", "start"]));
    b.stdout(&["exchange", "complete", &a_data]);
    a.stdout(&["exchange", "complete", &b_data]);
}

fn stderr(cli: &Cli, args: &[&str]) -> String {
    let output = cli.run(args);
    assert!(!output.status.success(), "{args:?} should fail");
    String::from_utf8_lossy(&output.stderr).to_string()
}

// @internal
#[test]
fn delivery_commands_report_an_empty_store() {
    let cli = initialized("Alice");

    let status = cli.stdout(&["delivery", "status"]);
    assert!(status.contains("Delivery Status"), "{status}");
    assert!(status.contains("Queued:     0"), "{status}");
    assert!(!status.contains("Next retry:"), "{status}");
    assert!(
        cli.stdout(&["delivery", "list"])
            .contains("No delivery records found.")
    );
    assert!(
        cli.stdout(&["delivery", "retry"])
            .contains("No retries due.")
    );
    assert!(
        cli.stdout(&["delivery", "cleanup"])
            .contains("Cleanup complete: 0 expired, 0 removed")
    );
    assert!(
        !cli.stdout(&["delivery", "translate", "connection_timeout"])
            .trim()
            .is_empty()
    );
}

// @internal
#[test]
fn two_contacts_with_one_name_are_flagged_and_can_be_dismissed() {
    let alice = initialized("Alice");
    for _ in 0..2 {
        exchange(&alice, &initialized("Bob Jones"));
    }

    let found = alice.stdout(&["contacts", "duplicates"]);
    assert!(
        found.contains("Potential duplicate contacts (1):"),
        "{found}"
    );
    assert!(
        found.contains("  1. Bob Jones <-> Bob Jones (100% similar)"),
        "{found}"
    );
}

// @internal
#[test]
fn two_different_contacts_are_not_duplicates() {
    let alice = initialized("Alice");
    exchange(&alice, &initialized("Bob Jones"));
    exchange(&alice, &initialized("Zed Quark"));

    let found = alice.stdout(&["contacts", "duplicates"]);

    assert!(found.contains("No potential duplicates found."), "{found}");
}

// @internal
#[test]
fn a_contact_cannot_be_merged_with_or_dismissed_against_itself() {
    let alice = initialized("Alice");
    exchange(&alice, &initialized("Bob Jones"));

    assert!(
        stderr(&alice, &["contacts", "merge", "Bob Jones", "Bob Jones"])
            .contains("Cannot merge a contact with itself")
    );
    assert!(
        stderr(
            &alice,
            &["contacts", "dismiss-duplicate", "Bob Jones", "Bob Jones"]
        )
        .contains("Cannot dismiss a contact pair with itself")
    );
}

// @internal
#[test]
fn a_near_duplicate_pair_names_both_contacts() {
    let alice = initialized("Alice");
    exchange(&alice, &initialized("Bob Jones"));
    exchange(&alice, &initialized("Bob Jonas"));

    let found = alice.stdout(&["contacts", "duplicates"]);

    // The pair's order follows the contacts' random ids.
    assert!(
        found.contains("  1. Bob Jonas <-> Bob Jones (75% similar)")
            || found.contains("  1. Bob Jones <-> Bob Jonas (75% similar)"),
        "{found}"
    );
}

// @internal
#[test]
fn duplicates_need_at_least_two_contacts() {
    let alice = initialized("Alice");
    exchange(&alice, &initialized("Bob Jones"));

    let found = alice.stdout(&["contacts", "duplicates"]);

    assert!(
        found.contains("Need at least 2 contacts to check for duplicates."),
        "{found}"
    );
}
