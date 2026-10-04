// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the plain-text commands print: social networks, FAQs, contact
//! summaries, details and tables, cards (vauchi/private#505).

use std::process::{Command, Output};

use tempfile::TempDir;

struct Cli {
    data_dir: TempDir,
}

impl Cli {
    fn new() -> Self {
        Self {
            data_dir: TempDir::new().expect("temp dir"),
        }
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_vauchi"))
            .arg("--data-dir")
            .arg(self.data_dir.path())
            .arg("--relay")
            .arg("ws://127.0.0.1:8080")
            .args(args)
            .output()
            .expect("run vauchi")
    }

    fn stdout(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).to_string()
    }
}

fn exchange_data(output: &str) -> String {
    output
        .lines()
        .map(str::trim)
        .find(|line| {
            line.len() >= 20
                && line
                    .chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '+' | '/' | '='))
        })
        .unwrap_or_else(|| panic!("no exchange data in {output}"))
        .to_string()
}

fn exchanged_pair() -> (Cli, Cli) {
    let alice = Cli::new();
    alice.stdout(&["init", "Alice Smith"]);
    let alice_data = exchange_data(&alice.stdout(&["exchange", "start"]));
    let bob = Cli::new();
    bob.stdout(&["init", "Bob Jones"]);
    bob.stdout(&["card", "add", "phone", "Mobile", "+1-555-262-1234"]);
    let bob_data = exchange_data(&bob.stdout(&["exchange", "start"]));
    bob.stdout(&["exchange", "complete", &alice_data]);
    alice.stdout(&["exchange", "complete", &bob_data]);
    (alice, bob)
}

// @internal
#[test]
fn social_networks_are_listed_in_blocks_of_five() {
    let cli = Cli::new();

    let output = cli.stdout(&["social", "list"]);

    assert!(output.contains("Available Social Networks"), "{output}");
    assert!(output.contains("Use: vauchi card add social <network> <username>"));
    let body: Vec<&str> = output
        .lines()
        .skip_while(|line| !line.starts_with("──"))
        .skip(2)
        .take_while(|line| !line.starts_with("──"))
        .collect();
    assert!(body.len() > 11, "{output}");
    for (index, line) in body.iter().enumerate().take(22) {
        assert_eq!(line.is_empty(), index % 11 == 10, "line {index}: {line:?}");
    }
}

// @internal
#[test]
fn a_social_search_without_match_says_so() {
    let output = Cli::new().stdout(&["social", "list", "zzzznotanetwork"]);

    assert_eq!(
        output.trim(),
        "No social networks matching 'zzzznotanetwork'"
    );
}

// @internal
#[test]
fn faq_categories_list_their_counts() {
    let output = Cli::new().stdout(&["faq", "categories"]);

    for id in [
        "getting-started",
        "privacy",
        "recovery",
        "contacts",
        "updates",
        "features",
    ] {
        assert!(output.contains(id), "{id} missing: {output}");
    }
    assert!(output.contains("FAQs)"), "{output}");
    assert!(
        output.contains("Use: vauchi help category <name>"),
        "{output}"
    );
}

// @internal
#[test]
fn a_faq_category_lists_its_questions_and_an_unknown_one_is_an_error() {
    let cli = Cli::new();

    let privacy = cli.stdout(&["faq", "category", "privacy"]);
    assert!(privacy.contains(": Privacy"), "{privacy}");
    assert!(privacy.lines().count() > 5, "{privacy}");

    let unknown = cli.run(&["faq", "category", "nonsense"]);
    let stderr = String::from_utf8_lossy(&unknown.stderr);
    assert!(stderr.contains("✗ Unknown category: nonsense"), "{stderr}");
}

// @internal
#[test]
fn a_faq_shows_its_related_entries_only_when_it_has_some() {
    let cli = Cli::new();

    let related = cli.stdout(&["faq", "show", "faq-phone-lost"]);
    assert!(related.contains("Related: faq-recovery-setup"), "{related}");

    let unrelated = cli.stdout(&["faq", "show", "faq-multiple-devices"]);
    assert!(!unrelated.contains("Related:"), "{unrelated}");

    let missing = cli.run(&["faq", "show", "faq-none"]);
    assert!(String::from_utf8_lossy(&missing.stderr).contains("FAQ not found: faq-none"));
}

// @internal
#[test]
fn a_card_shows_icons_and_a_profile_url_only_for_social_fields() {
    let cli = Cli::new();
    cli.stdout(&["init", "Alice"]);
    cli.stdout(&["card", "add", "email", "Work", "alice@work.com"]);
    cli.stdout(&["card", "add", "social", "github", "octocat"]);
    cli.stdout(&["card", "add", "website", "github", "example.com"]);

    let card = cli.stdout(&["card", "show"]);

    assert!(card.contains("envelope"), "{card}");
    assert!(card.contains("https://github.com/octocat"), "{card}");
    assert!(!card.contains("https://github.com/example.com"), "{card}");
}

// @internal
#[test]
fn contact_search_numbers_its_results() {
    let (alice, _bob) = exchanged_pair();

    let output = alice.stdout(&["contacts", "search", "Bob"]);

    assert!(output.contains("1. Bob Jones"), "{output}");
}

// @internal
#[test]
fn contact_details_show_name_id_and_status() {
    let (alice, _bob) = exchanged_pair();
    let details = alice.stdout(&["contacts", "show", "Bob Jones"]);

    assert!(details.contains("Bob Jones"), "{details}");
    assert!(details.contains("Status: Not verified"), "{details}");
    assert!(details.contains("ID: "), "{details}");
}

// @internal
#[test]
fn hidden_contacts_are_listed_as_a_numbered_table() {
    let (alice, _bob) = exchanged_pair();
    alice.stdout(&["contacts", "hide-contact", "Bob Jones"]);

    let table = alice.stdout(&["contacts", "list-hidden"]);

    let row = table
        .lines()
        .find(|line| line.contains("Bob Jones"))
        .unwrap_or_else(|| panic!("{table}"));
    assert!(row.trim_start().starts_with("│ 1 │"), "{row}");
    assert!(row.contains("not verified"), "{row}");
}

// @internal
#[test]
fn a_faq_category_is_headed_by_the_localized_faq_title() {
    let output = Cli::new().stdout(&["faq", "category", "privacy"]);

    assert!(
        output.contains("FAQ: Privacy"),
        "{output}"
    );
}

// @internal
#[test]
fn activity_lists_a_card_edit() {
    let cli = Cli::new();
    cli.stdout(&["init", "Alice"]);
    cli.stdout(&["card", "add", "email", "Work", "alice@work.com"]);

    let output = cli.stdout(&["activity"]);

    assert!(output.contains("You Updated Your Card"), "{output}");
}

