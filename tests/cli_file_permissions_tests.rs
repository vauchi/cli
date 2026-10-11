// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Files holding backups or personal data are readable by their owner only
//! (private#285): 0o600 whatever the umask, from the moment they are
//! created.

#![cfg(unix)]

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use common::{Cli, exchange_data};

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path)
        .expect("file written")
        .permissions()
        .mode()
        & 0o777
}

fn assert_owner_only(path: &Path) {
    assert_eq!(
        mode(path),
        0o600,
        "{} is readable beyond its owner",
        path.display()
    );
}

fn ok(output: std::process::Output) {
    assert!(
        output.status.success(),
        "failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// @scenario: backup_recovery :: Create encrypted backup
#[test]
fn test_backups_are_owner_only() {
    let cli = Cli::new();
    cli.stdout(&["init", "Alice Smith"]);
    let env = [("VAUCHI_BACKUP_PASSWORD", "correct-horse-battery-1")];
    let identity = cli.data_dir().join("identity.backup");
    let full = cli.data_dir().join("full.backup");

    ok(cli.run_with_env(&["export", identity.to_str().unwrap()], &env));
    ok(cli.run_with_env(&["export", full.to_str().unwrap(), "--full"], &env));

    assert_owner_only(&identity);
    assert_owner_only(&full);
}

// @scenario: privacy_compliance :: Export all my data
#[test]
fn test_personal_data_exports_are_owner_only() {
    let cli = Cli::new();
    cli.stdout(&["init", "Alice Smith"]);
    let plain = cli.data_dir().join("export.json");
    let encrypted = cli.data_dir().join("export.enc");

    ok(cli.run_with_env(&["gdpr", "export", plain.to_str().unwrap()], &[]));
    ok(cli.run_with_env(
        &["gdpr", "export", encrypted.to_str().unwrap(), "--encrypt"],
        &[("VAUCHI_EXPORT_PASSWORD", "export-password")],
    ));

    assert_owner_only(&plain);
    assert_owner_only(&encrypted);
}

// @scenario: contact_actions :: Export contact as vCard
#[test]
fn test_vcard_export_is_owner_only() {
    let alice = Cli::new();
    alice.stdout(&["init", "Alice Smith"]);
    let bob = Cli::new();
    bob.stdout(&["init", "Bob Jones"]);
    let alice_data = exchange_data(&alice.stdout(&["exchange", "start"]));
    let bob_data = exchange_data(&bob.stdout(&["exchange", "start"]));
    bob.stdout(&["exchange", "complete", &alice_data]);
    alice.stdout(&["exchange", "complete", &bob_data]);
    let vcf = alice.data_dir().join("bob.vcf");

    ok(alice.run_with_env(
        &["contacts", "export", "Bob Jones", vcf.to_str().unwrap()],
        &[],
    ));

    assert_owner_only(&vcf);
}
