// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! `vauchi export` writes the full backup unless told otherwise, and
//! `vauchi import` reads either kind without being told which (ADR-058
//! amendment 2026-10-28; private#1060).

use std::fs;

use super::CliTestContext;

const PASSWORD: &str = "correct horse battery staple 42";
const ENV: [(&str, &str); 1] = [("VAUCHI_BACKUP_PASSWORD", PASSWORD)];

fn source_with_label(label: &str) -> CliTestContext {
    let source = CliTestContext::new();
    source.init("Alice source");
    source.run_success(&["labels", "create", label]);
    source
}

// @internal
#[test]
fn export_without_flags_writes_a_full_backup_that_import_restores() {
    let source = source_with_label("Climbing club");
    let backup = source.data_dir.path().join("default.vauchi");
    source.run_success_env(&["export", backup.to_str().unwrap()], &ENV);

    let target = CliTestContext::new();
    target.run_success_env(&["import", backup.to_str().unwrap(), "--yes"], &ENV);

    let labels = target.run_success(&["labels", "list"]);
    assert!(
        labels.contains("Climbing club"),
        "a default export must carry labels, as a full backup does: {labels}"
    );
}

// @internal
#[test]
fn export_identity_only_carries_the_identity_and_nothing_else() {
    let source = source_with_label("Climbing club");
    let backup = source.data_dir.path().join("identity.vauchi");
    source.run_success_env(
        &["export", backup.to_str().unwrap(), "--identity-only"],
        &ENV,
    );

    let target = CliTestContext::new();
    target.run_success_env(&["import", backup.to_str().unwrap(), "--yes"], &ENV);

    let card = target.run_success(&["card", "show"]);
    assert!(card.contains("Alice source"), "{card}");
    let labels = target.run_success(&["labels", "list"]);
    assert!(
        !labels.contains("Climbing club"),
        "an identity-only backup must not carry labels: {labels}"
    );
}

// @internal
#[test]
fn import_refuses_a_file_that_is_no_vauchi_backup() {
    let target = CliTestContext::new();
    let junk = target.data_dir.path().join("notes.txt");
    fs::write(&junk, "shopping list: milk, eggs").unwrap();

    let output = target.run_env(&["import", junk.to_str().unwrap(), "--yes"], &ENV);

    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("not a Vauchi backup"),
        "the refusal must say what is wrong: {error}"
    );
}

// A full import replaces the database, so it needs the profile to itself
// even when the kind comes from the file instead of `--full` (#696).
// @internal
#[cfg(unix)]
#[test]
fn a_detected_full_import_still_refuses_a_profile_used_by_another_command() {
    let source = source_with_label("Climbing club");
    let backup = source.data_dir.path().join("default.vauchi");
    source.run_success_env(&["export", backup.to_str().unwrap()], &ENV);
    let target = CliTestContext::new();
    target.init("Bob original");
    let access = fs::File::open(target.data_dir.path()).unwrap();
    access.lock_shared().unwrap();

    let output = target.run_env(&["import", backup.to_str().unwrap(), "--yes"], &ENV);

    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("Another Vauchi command is using this data directory"),
        "{error}"
    );
}
