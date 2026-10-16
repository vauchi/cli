// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Two own entries may share a label (private#649); the CLI names an entry by
//! its id or by a label only one entry has, and refuses an ambiguous label
//! with the entries' ids (private#651).

use std::process::{Command, Output};
use tempfile::TempDir;

struct Cli {
    data_dir: TempDir,
}

impl Cli {
    fn with_two_phones() -> Self {
        let cli = Cli {
            data_dir: TempDir::new().expect("temp dir"),
        };
        for args in [
            &["init", "Alice"][..],
            &["card", "add", "phone", "Phone", "5550101"],
            &["card", "add", "phone", "Phone", "5550202"],
        ] {
            let out = cli.run(args);
            assert!(out.status.success(), "{args:?}: {}", text(&out.stderr));
        }
        cli
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_vauchi"))
            .arg("--data-dir")
            .arg(self.data_dir.path())
            .arg("--relay")
            .arg("ws://127.0.0.1:9")
            .args(args)
            .output()
            .expect("run vauchi")
    }

    /// The own card as `--raw` prints it: (id, value) per entry.
    fn entries(&self) -> Vec<(String, String)> {
        let out = self.run(&["--raw", "card", "show"]);
        let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("raw card");
        json["fields"]
            .as_array()
            .expect("fields")
            .iter()
            .map(|f| {
                (
                    f["id"].as_str().unwrap_or_default().to_string(),
                    f["value"].as_str().unwrap_or_default().to_string(),
                )
            })
            .collect()
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

// @internal
#[test]
fn the_raw_card_names_each_entry_by_id() {
    let cli = Cli::with_two_phones();

    let entries = cli.entries();

    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|(id, _)| !id.is_empty()), "{entries:?}");
    assert_ne!(entries[0].0, entries[1].0);
}

// @scenario: contact_card_management :: an ambiguous label is refused with the entries' ids
// @internal
#[test]
fn editing_a_label_two_entries_share_is_refused_with_their_ids() {
    let cli = Cli::with_two_phones();
    let before = cli.entries();

    let out = cli.run(&["card", "edit", "Phone", "5550303"]);

    assert!(!out.status.success(), "{}", text(&out.stdout));
    let err = text(&out.stderr);
    assert!(
        before.iter().all(|(id, _)| err.contains(id.as_str())),
        "the refusal names both ids: {err}"
    );
    assert_eq!(cli.entries(), before, "nothing changed");
}

// @scenario: contact_card_management :: an entry is edited by its id
// @internal
#[test]
fn an_entry_id_edits_that_entry_only() {
    let cli = Cli::with_two_phones();
    let before = cli.entries();
    let (second_id, _) = before[1].clone();

    let out = cli.run(&["card", "edit", &second_id, "5550303"]);

    assert!(out.status.success(), "{}", text(&out.stderr));
    let after = cli.entries();
    assert_eq!(after[0], before[0], "its namesake is untouched");
    assert_eq!(after[1], (second_id, "5550303".to_string()));
}
