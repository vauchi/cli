// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The binary run in an isolated data directory, shared by the test files
//! that assert on what commands print.

#![allow(dead_code)]

use std::process::{Command, Output};

use tempfile::TempDir;

pub struct Cli {
    data_dir: TempDir,
}

impl Cli {
    pub fn new() -> Self {
        Self {
            data_dir: TempDir::new().expect("temp dir"),
        }
    }

    pub fn data_dir(&self) -> &std::path::Path {
        self.data_dir.path()
    }

    pub fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_vauchi"))
            .arg("--data-dir")
            .arg(self.data_dir.path())
            .arg("--relay")
            .arg("ws://127.0.0.1:8080")
            .args(args)
            .output()
            .expect("run vauchi")
    }

    /// Stdout of a command that must succeed.
    pub fn stdout(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).to_string()
    }
}

/// The QR data line `exchange start` prints.
pub fn exchange_data(output: &str) -> String {
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

/// Alice and Bob, each holding the other as a contact.
pub fn exchanged_pair() -> (Cli, Cli) {
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
