// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! `vauchi exchange link` (private#670): the CLI shares or accepts the same
//! remote-exchange link the apps' Link mode shows. No relay runs here, so
//! every exchange stays unfinished; the round trip lives in e2e.

use std::process::{Command, Output};
use tempfile::TempDir;

struct Cli {
    data_dir: TempDir,
}

impl Cli {
    fn with_identity(name: &str) -> Self {
        let cli = Cli {
            data_dir: TempDir::new().expect("temp dir"),
        };
        let out = cli.run(&["init", name]);
        assert!(out.status.success(), "init: {}", stderr(&out));
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

    fn share_link(&self) -> String {
        let out = self.run(&["exchange", "link", "--wait-secs", "0"]);
        stdout(&out)
            .split_whitespace()
            .find(|word| word.starts_with("https://vauchi.app/x#"))
            .unwrap_or_else(|| panic!("no share link in: {}", stdout(&out)))
            .to_string()
    }
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

// @scenario: contact_exchange :: a CLI user shares a link for a remote exchange
// @internal
#[test]
fn exchange_link_prints_this_devices_share_link() {
    let alice = Cli::with_identity("Alice");

    let link = alice.share_link();

    assert!(link.contains("pk="), "{link}");
    assert!(link.contains("n="), "{link}");
}

// An exchange the peer never answered is not a success: a script must be
// able to tell it apart from one that completed.
// @internal
#[test]
fn an_unanswered_link_exchange_fails_after_its_wait() {
    let alice = Cli::with_identity("Alice");

    let out = alice.run(&["exchange", "link", "--wait-secs", "0"]);

    assert!(!out.status.success());
    assert!(stderr(&out).contains("not completed"), "{}", stderr(&out));
}

// @scenario: contact_exchange :: a CLI user accepts a link from another user
// @internal
#[test]
fn exchange_link_accepts_another_users_link() {
    let alice = Cli::with_identity("Alice");
    let bob = Cli::with_identity("Bob");
    let link = alice.share_link();

    let out = bob.run(&["exchange", "link", &link, "--wait-secs", "0"]);

    let err = stderr(&out);
    assert!(
        !err.contains("not a Vauchi exchange link"),
        "a real link is accepted: {err}"
    );
    assert!(err.contains("not completed"), "{err}");
}

// CC-14: the link is untrusted input.
// @internal
#[test]
fn exchange_link_rejects_anything_that_is_not_an_exchange_link() {
    let bob = Cli::with_identity("Bob");

    for junk in [
        "",
        "hello",
        "https://example.com/x#pk=a&n=b",
        "https://vauchi.app/x#",
        "vauchi://exchange",
        "https://vauchi.app/x#pk=\0&n=\0",
    ] {
        let out = bob.run(&["exchange", "link", junk, "--wait-secs", "0"]);
        assert!(!out.status.success(), "{junk:?} must fail");
        assert!(
            stderr(&out).contains("not a Vauchi exchange link"),
            "{junk:?}: {}",
            stderr(&out)
        );
    }
}
