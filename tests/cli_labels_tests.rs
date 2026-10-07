// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Label membership and field visibility as `labels show` reports them
//! (vauchi/private#505).

mod common;

use common::exchanged_pair;

fn section<'a>(output: &'a str, header: &str) -> Vec<&'a str> {
    output
        .lines()
        .skip_while(|line| !line.contains(header))
        .skip(1)
        .take_while(|line| line.starts_with("  - "))
        .collect()
}

// @internal
#[test]
fn a_contact_joins_and_leaves_a_label_by_name() {
    let (alice, _bob) = exchanged_pair();
    alice.stdout(&["labels", "create", "Family"]);

    let added = alice.stdout(&["labels", "add-contact", "Family", "Bob"]);
    assert!(
        added.contains("Added 'Bob Jones' to label 'Family'"),
        "{added}"
    );
    let shown = alice.stdout(&["labels", "show", "Family"]);
    let members = section(&shown, "Contacts:");
    assert_eq!(members.len(), 1, "{shown}");
    assert!(members[0].starts_with("  - Bob Jones ("), "{shown}");

    let removed = alice.stdout(&["labels", "remove-contact", "Family", "Bob"]);
    assert!(
        removed.contains("Removed 'Bob Jones' from label 'Family'"),
        "{removed}"
    );
    let shown = alice.stdout(&["labels", "show", "Family"]);
    assert!(section(&shown, "Contacts:").is_empty(), "{shown}");
}

// @internal
#[test]
fn a_field_is_shown_to_and_hidden_from_a_label_by_its_name() {
    let (alice, _bob) = exchanged_pair();
    alice.stdout(&["card", "add", "email", "Work", "alice@work.com"]);
    alice.stdout(&["labels", "create", "Family"]);

    let shown = alice.stdout(&["labels", "show-field", "Family", "work"]);
    assert!(
        shown.contains("Field 'Work' is now visible to contacts in 'Family'"),
        "{shown}"
    );
    let detail = alice.stdout(&["labels", "show", "Family"]);
    assert_eq!(
        section(&detail, "Visible fields:"),
        ["  - Work"],
        "{detail}"
    );

    let hidden = alice.stdout(&["labels", "hide-field", "Family", "Work"]);
    assert!(
        hidden.contains("Field 'Work' is now hidden from contacts in 'Family'"),
        "{hidden}"
    );
    let detail = alice.stdout(&["labels", "show", "Family"]);
    assert!(section(&detail, "Visible fields:").is_empty(), "{detail}");
}

// The presentation-override block reads from the locale like the rest of
// `labels show`; it printed hardcoded English (vauchi/private#544). The
// English output keeps its wording.
// @internal
#[test]
fn the_presentation_override_block_follows_the_locale() {
    let alice = common::Cli::new();
    alice.stdout(&["init", "Alice"]);
    alice.stdout(&["labels", "create", "Family"]);
    alice.stdout(&["labels", "set-name", "Family", "Ali"]);

    let german = alice.stdout(&["--locale", "de", "labels", "show", "Family"]);
    let english = alice.stdout(&["--locale", "en", "labels", "show", "Family"]);

    assert!(german.contains("Angepasste Darstellung:"), "{german}");
    assert!(german.contains("  Name: Ali"), "{german}");
    assert!(!german.contains("Presentation overrides:"), "{german}");
    for line in [
        "Presentation overrides:",
        "  Name: Ali",
        "  Bio: -",
        "  Avatar: -",
    ] {
        assert!(english.contains(line), "{line:?} not in {english}");
    }
}
