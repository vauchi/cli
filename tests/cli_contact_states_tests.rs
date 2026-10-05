// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The contact state toggles and their "already so" answers, removal,
//! verification, import, deletion and the contact limit, through the
//! binary (vauchi/private#505).

mod common;

use common::{Cli, exchanged_pair};

fn says(cli: &Cli, args: &[&str], expected: &str) {
    let output = cli.stdout(args);
    assert!(output.contains(expected), "{args:?}: {output}");
}

fn fails_with(cli: &Cli, args: &[&str], expected: &str) {
    let output = cli.run(args);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{args:?} should fail");
    assert!(stderr.contains(expected), "{args:?}: {stderr}");
}

fn bob_id(alice: &Cli) -> String {
    let raw = alice.stdout(&["--raw", "contacts", "list"]);
    let contacts: serde_json::Value = serde_json::from_str(&raw).expect("JSON document");
    contacts
        .as_array()
        .expect("array")
        .iter()
        .find(|c| c["display_name"] == "Bob Jones")
        .and_then(|c| c["id"].as_str())
        .unwrap_or_else(|| panic!("no Bob in {raw}"))
        .to_string()
}

const VCF: &str = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Carol Imported\r\nEND:VCARD\r\n\
BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Dave Imported\r\nEND:VCARD\r\n";

// @internal
#[test]
fn each_toggle_answers_when_there_is_nothing_to_undo_and_undoes_otherwise() {
    let (alice, _bob) = exchanged_pair();

    says(
        &alice,
        &["contacts", "unblock", "Bob"],
        "Bob Jones is not blocked",
    );
    alice.stdout(&["contacts", "block", "Bob"]);
    says(
        &alice,
        &["contacts", "unblock", "Bob"],
        "Unblocked Bob Jones",
    );

    says(
        &alice,
        &["contacts", "unfavorite", "Bob"],
        "Bob Jones is not a favorite",
    );
    alice.stdout(&["contacts", "favorite", "Bob"]);
    says(
        &alice,
        &["contacts", "unfavorite", "Bob"],
        "Removed Bob Jones from favorites",
    );

    says(
        &alice,
        &["contacts", "untrust", "Bob"],
        "Bob Jones is not recovery-trusted",
    );
    // Only in-person verified contacts can be trusted for recovery.
    alice.stdout(&["contacts", "verify", "Bob"]);
    alice.stdout(&["contacts", "trust", "Bob"]);
    let untrusted = alice.stdout(&["contacts", "untrust", "Bob"]);
    assert!(
        untrusted.contains("Removed recovery trust from Bob Jones"),
        "{untrusted}"
    );
    assert!(
        untrusted.contains("Only 0 trusted contact(s) remaining"),
        "{untrusted}"
    );

    says(
        &alice,
        &["contacts", "unhide-contact", "Bob"],
        "Bob Jones is not hidden",
    );
    alice.stdout(&["contacts", "hide-contact", "Bob"]);
    says(
        &alice,
        &["contacts", "unhide-contact", "Bob"],
        "Unhidden Bob Jones — now visible in contact list",
    );

    // An archived contact is no longer found by name, only by id.
    let id = bob_id(&alice);
    alice.stdout(&["contacts", "archive", "Bob"]);
    says(
        &alice,
        &["contacts", "unarchive", &id],
        "Unarchived contact: Bob Jones",
    );
}

// @internal
#[test]
fn verifying_shows_both_fingerprints_and_marks_the_contact() {
    let (alice, _bob) = exchanged_pair();

    let output = alice.stdout(&["contacts", "verify", "Bob"]);

    assert!(
        output.contains("Their fingerprint (Bob Jones):"),
        "{output}"
    );
    assert!(
        output.contains("Verified fingerprint for Bob Jones"),
        "{output}"
    );
    says(
        &alice,
        &["contacts", "verify", "Bob"],
        "Bob Jones is already verified",
    );
}

// @internal
#[test]
fn removing_by_id_names_the_contact_and_an_unknown_id_warns() {
    let (alice, _bob) = exchanged_pair();
    let id = bob_id(&alice);

    let removed = alice.stdout(&["contacts", "remove", &id]);
    assert!(removed.contains("Removed contact: Bob Jones"), "{removed}");
    let again = alice.stdout(&["contacts", "remove", &id]);
    assert!(
        again.contains(&format!("Contact '{id}' not found")),
        "{again}"
    );
}

/// The side of a fresh pair that can send: the responder cannot until the
/// initiator's first message arrives, and which side that is varies
/// (vauchi/private#523). Returns that side and the other side's name.
fn sending_side<'a>(alice: &'a Cli, bob: &'a Cli, field: &str) -> (&'a Cli, &'static str) {
    for (me, peer) in [(alice, "Bob"), (bob, "Alice")] {
        if me
            .run(&["contacts", "unhide", peer, field])
            .status
            .success()
        {
            return (me, peer);
        }
    }
    panic!("neither side of the pair can send");
}

// @internal
#[test]
fn a_field_shown_to_one_contact_is_reported() {
    let (alice, bob) = exchanged_pair();
    for side in [&alice, &bob] {
        side.stdout(&["card", "add", "email", "Work", "a@work.ch"]);
    }
    let (me, peer) = sending_side(&alice, &bob, "Work");

    let shown = me.stdout(&["contacts", "unhide", peer, "Work"]);

    assert!(
        shown.contains(&format!("'Work' field is now visible to {peer}")),
        "{shown}"
    );
}

// @internal
#[test]
fn imported_contacts_can_be_deleted_and_exchanged_ones_cannot() {
    let (alice, _bob) = exchanged_pair();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("contacts.vcf");
    std::fs::write(&file, VCF).unwrap();

    let imported = alice.stdout(&["contacts", "import-vcf", file.to_str().unwrap()]);
    assert!(imported.contains("Imported 2 contacts"), "{imported}");
    assert!(!imported.contains("Skipped"), "{imported}");

    says(
        &alice,
        &["contacts", "delete", "Carol Imported", "--yes"],
        "Deleted contact: Carol Imported",
    );
    fails_with(
        &alice,
        &["contacts", "delete", "Bob", "--yes"],
        "Only imported contacts can be deleted",
    );
}

// @internal
#[test]
fn the_limit_reports_room_left_and_warns_when_lowered_below_the_count() {
    let (alice, _bob) = exchanged_pair();

    let raised = alice.stdout(&["contacts", "limit", "--set", "3"]);
    assert!(raised.contains("Contact limit set to 3"), "{raised}");
    assert!(!raised.contains("exceeds the new limit"), "{raised}");
    let room = alice.stdout(&["contacts", "limit"]);
    assert!(room.contains("Contact limit: 1 / 3"), "{room}");
    assert!(room.contains("2 contact slots remaining."), "{room}");

    let lowered = alice.stdout(&["contacts", "limit", "--set", "1"]);
    assert!(!lowered.contains("exceeds the new limit"), "{lowered}");
    let full = alice.stdout(&["contacts", "limit"]);
    assert!(full.contains("Contact limit reached."), "{full}");

    alice.stdout(&["contacts", "limit", "--set", "2"]);
    let exact = alice.stdout(&["contacts", "limit", "--set", "1"]);
    assert!(!exact.contains("exceeds the new limit"), "{exact}");
}

// @internal
#[test]
fn visibility_shows_inherited_and_overridden_fields() {
    let (alice, bob) = exchanged_pair();
    for side in [&alice, &bob] {
        side.stdout(&["card", "add", "email", "Before", "b@x.ch"]);
        side.stdout(&["card", "add", "email", "After", "a@x.ch"]);
    }
    let (me, peer) = sending_side(&alice, &bob, "Before");
    me.stdout(&["contacts", "clear-override", peer, "Before"]);

    let default = me.stdout(&["contacts", "visibility", peer]);
    assert!(
        default.contains("  ✓ visible Before [inherited]: b@x.ch"),
        "{default}"
    );
    assert!(
        default.contains("All fields are visible to this contact (default)."),
        "{default}"
    );

    me.stdout(&["contacts", "hide", peer, "After"]);
    let overridden = me.stdout(&["contacts", "visibility", peer]);
    assert!(
        overridden.contains("  ✗ hidden After [override]: a@x.ch"),
        "{overridden}"
    );
    assert!(!overridden.contains("(default)"), "{overridden}");
}
