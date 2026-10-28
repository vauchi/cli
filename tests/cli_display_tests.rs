// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What the plain-text commands print: social networks, FAQs, contact
//! summaries, details and tables, cards (vauchi/private#505).

mod common;

use common::{Cli, exchanged_pair};

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

// @scenario: help_faq :: View FAQ categories from CLI
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
        output.contains("Use: vauchi faq category <name>"),
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

// @scenario: help_faq :: Access FAQs from CLI
#[test]
fn faq_list_prints_every_bundled_question() {
    let output = Cli::new().stdout(&["faq", "list"]);

    let faqs = vauchi_app::help::get_faqs();
    assert_eq!(faqs.len(), 14);
    for faq in faqs {
        assert!(output.contains(&faq.question), "{}: {output}", faq.id);
    }
}

// @scenario: help_faq :: Search FAQs from CLI
#[test]
fn a_faq_query_prints_the_matching_questions_and_no_others() {
    let output = Cli::new().stdout(&["faq", "list", "encryption"]);

    let matching = vauchi_app::help::search_faqs("encryption");
    assert!(!matching.is_empty());
    for faq in vauchi_app::help::get_faqs() {
        let expected = matching.iter().any(|m| m.id == faq.id);
        assert_eq!(
            output.contains(&faq.question),
            expected,
            "{}: {output}",
            faq.id
        );
    }
}

// @scenario: help_faq :: View specific FAQ from CLI
#[test]
fn faq_show_prints_the_question_the_whole_answer_and_related_ids() {
    let output = Cli::new().stdout(&["faq", "show", "faq-phone-lost"]);

    let faq = vauchi_app::help::get_faq_by_id("faq-phone-lost").unwrap();
    let printed_words: Vec<&str> = output.split_whitespace().collect();
    let answer_words: Vec<&str> = faq.answer.split_whitespace().collect();
    assert!(output.contains(&faq.question), "{output}");
    assert!(
        printed_words
            .windows(answer_words.len())
            .any(|w| w == answer_words),
        "the whole answer, wrapped: {output}"
    );
    assert!(output.contains("Related: faq-recovery-setup"), "{output}");
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

    assert!(output.contains("FAQ: Privacy"), "{output}");
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
