// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::{self, File};

use super::CliTestContext;

const PASSWORD: &str = "correct horse battery staple 42";

// @internal
#[test]
fn backup_overwrite_legacy_cleanup_failure_preserves_the_existing_installation() {
    let source = CliTestContext::new();
    source.init("Alice source");
    let backup = source.data_dir.path().join("full.vauchi");
    source.run_success_env(
        &["export", backup.to_str().unwrap(), "--full"],
        &[("VAUCHI_BACKUP_PASSWORD", PASSWORD)],
    );

    let target = CliTestContext::new();
    target.init("Bob original");
    target.run_success(&["labels", "create", "Original group"]);
    fs::create_dir(target.data_dir.path().join("identity.json")).unwrap();
    let output = target.run_env(
        &["import", backup.to_str().unwrap(), "--full", "--yes"],
        &[("VAUCHI_BACKUP_PASSWORD", PASSWORD)],
    );

    assert!(!output.status.success());
    assert!(
        target.data_dir.path().join("vauchi.db").is_file(),
        "a failed full overwrite must retain the existing database"
    );
    fs::remove_dir(target.data_dir.path().join("identity.json")).unwrap();
    let card = target.run_success(&["card", "show"]);
    assert!(card.contains("Bob original"), "{card}");
    assert!(!card.contains("Alice source"), "{card}");
    let labels = target.run_success(&["labels", "list"]);
    assert!(labels.contains("Original group"), "{labels}");
}

// @internal
#[test]
fn backup_overwrite_refuses_to_replace_a_profile_used_by_another_command() {
    let source = CliTestContext::new();
    source.init("Alice source");
    let backup = source.data_dir.path().join("full.vauchi");
    source.run_success_env(
        &["export", backup.to_str().unwrap(), "--full"],
        &[("VAUCHI_BACKUP_PASSWORD", PASSWORD)],
    );
    let target = CliTestContext::new();
    target.init("Bob original");
    let access = File::open(target.data_dir.path()).unwrap();
    access.lock_shared().unwrap();

    let output = target.run_env(
        &["import", backup.to_str().unwrap(), "--full", "--yes"],
        &[("VAUCHI_BACKUP_PASSWORD", PASSWORD)],
    );

    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("Another Vauchi command is using this data directory"),
        "{error}"
    );
    let card = target.run_success(&["card", "show"]);
    assert!(card.contains("Bob original"), "{card}");
    assert!(!card.contains("Alice source"), "{card}");
}

// @internal
#[test]
fn backup_overwrite_exclusive_restore_access_blocks_other_cli_commands() {
    let target = CliTestContext::new();
    target.init("Bob original");
    let access = File::open(target.data_dir.path()).unwrap();
    access.lock().unwrap();

    let output = target.run(&["card", "show"]);

    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("Another Vauchi command is using this data directory"),
        "{error}"
    );
    drop(access);
    let card = target.run_success(&["card", "show"]);
    assert!(card.contains("Bob original"), "{card}");
}

// @internal
#[test]
fn backup_overwrite_shared_access_keeps_ordinary_cli_commands_available() {
    let target = CliTestContext::new();
    target.init("Bob original");
    let access = File::open(target.data_dir.path()).unwrap();
    access.lock_shared().unwrap();

    let card = target.run_success(&["card", "show"]);

    assert!(card.contains("Bob original"), "{card}");
}

// @internal
#[test]
fn backup_overwrite_retains_its_input_backup_and_unrelated_profile_files() {
    let source = CliTestContext::new();
    source.init("Alice replacement");
    source.run_success(&["labels", "create", "Restored group"]);
    let target = CliTestContext::new();
    target.init("Bob original");
    target.run_success(&["labels", "create", "Old group"]);
    let saved = target.data_dir.path().join("input-backup.vauchi");
    source.run_success_env(
        &["export", saved.to_str().unwrap(), "--full"],
        &[("VAUCHI_BACKUP_PASSWORD", PASSWORD)],
    );
    let backup = fs::read(&saved).unwrap();
    let marker = target.data_dir.path().join("keep.txt");
    fs::write(&marker, b"unrelated profile file").unwrap();

    target.run_success_env(
        &["import", saved.to_str().unwrap(), "--full", "--yes"],
        &[("VAUCHI_BACKUP_PASSWORD", PASSWORD)],
    );

    let card = target.run_success(&["card", "show"]);
    assert!(card.contains("Alice replacement"), "{card}");
    assert!(!card.contains("Bob original"), "{card}");
    let labels = target.run_success(&["labels", "list"]);
    assert!(labels.contains("Restored group"), "{labels}");
    assert!(!labels.contains("Old group"), "{labels}");
    assert_eq!(fs::read(saved).unwrap(), backup);
    assert_eq!(fs::read(marker).unwrap(), b"unrelated profile file");
}

// @internal
#[test]
fn backup_overwrite_refuses_a_database_symlink_without_replacing_it() {
    let source = CliTestContext::new();
    source.init("Alice replacement");
    let backup = source.data_dir.path().join("full.vauchi");
    source.run_success_env(
        &["export", backup.to_str().unwrap(), "--full"],
        &[("VAUCHI_BACKUP_PASSWORD", PASSWORD)],
    );
    let target = CliTestContext::new();
    target.init("Bob original");
    let database = target.data_dir.path().join("vauchi.db");
    let original = target.data_dir.path().join("original.db");
    fs::rename(&database, &original).unwrap();
    std::os::unix::fs::symlink(&original, &database).unwrap();

    let output = target.run_env(
        &["import", backup.to_str().unwrap(), "--full", "--yes"],
        &[("VAUCHI_BACKUP_PASSWORD", PASSWORD)],
    );

    assert!(!output.status.success());
    assert_eq!(fs::read_link(database).unwrap(), original);
    let card = target.run_success(&["card", "show"]);
    assert!(card.contains("Bob original"), "{card}");
    assert!(!card.contains("Alice replacement"), "{card}");
}
