// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! A full import replaces the database file. A Vauchi command that already
//! has the profile open would go on using the replaced file, so the import
//! waits for the data directory to itself, and ordinary commands share it
//! (vauchi/private#696).

#![cfg(unix)]

mod common;

use std::fs::File;

use common::Cli;

const ENV: [(&str, &str); 1] = [("VAUCHI_BACKUP_PASSWORD", "correct-horse-battery-1")];

fn alice_and_bobs_backup() -> (Cli, Cli, String) {
    let alice = Cli::new();
    alice.stdout(&["init", "Alice Smith"]);
    let bob = Cli::new();
    bob.stdout(&["init", "Bob Jones"]);
    let backup = bob.data_dir().join("bob-full.backup");
    let exported = bob.run_with_env(&["export", backup.to_str().unwrap(), "--full"], &ENV);
    assert!(
        exported.status.success(),
        "export failed: {}",
        String::from_utf8_lossy(&exported.stderr)
    );
    (alice, bob, backup.to_str().unwrap().to_owned())
}

/// Another process holding the profile keeps a full import out, and the
/// installation is left as it was; once it lets go the same import runs.
// @internal
#[test]
fn a_full_import_waits_until_no_other_command_uses_the_profile() {
    let (alice, _bob, backup) = alice_and_bobs_backup();
    let other_command = File::open(alice.data_dir()).unwrap();
    other_command.lock_shared().unwrap();

    let refused = alice.run_with_env(&["import", &backup, "--full", "--yes"], &ENV);

    assert!(
        !refused.status.success(),
        "the import ran beside another command"
    );
    assert!(
        String::from_utf8_lossy(&refused.stderr)
            .contains("Another Vauchi command is using this data directory"),
        "unexpected error: {}",
        String::from_utf8_lossy(&refused.stderr)
    );
    assert!(alice.stdout(&["card", "show"]).contains("Alice Smith"));

    drop(other_command);
    let imported = alice.run_with_env(&["import", &backup, "--full", "--yes"], &ENV);

    assert!(
        imported.status.success(),
        "import failed after release: {}",
        String::from_utf8_lossy(&imported.stderr)
    );
    assert!(alice.stdout(&["card", "show"]).contains("Bob Jones"));
}

/// Ordinary commands share the profile, so one open command does not lock
/// out the next.
// @internal
#[test]
fn ordinary_commands_share_the_profile() {
    let alice = Cli::new();
    alice.stdout(&["init", "Alice Smith"]);
    let other_command = File::open(alice.data_dir()).unwrap();
    other_command.lock_shared().unwrap();

    let shown = alice.run(&["card", "show"]);

    assert!(
        shown.status.success(),
        "a read was locked out: {}",
        String::from_utf8_lossy(&shown.stderr)
    );
    assert!(String::from_utf8_lossy(&shown.stdout).contains("Alice Smith"));
}
