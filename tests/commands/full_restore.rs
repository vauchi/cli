// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;

use tempfile::TempDir;
use vauchi_core::{SymmetricKey, Vauchi, VauchiConfig};

use super::restore;
use crate::config::CliConfig;

const PASSWORD: &str = "correct horse battery staple 42";

fn target() -> (TempDir, CliConfig, SymmetricKey, String) {
    let directory = TempDir::new().unwrap();
    let config = CliConfig {
        data_dir: directory.path().to_path_buf(),
        relay_url: "ws://127.0.0.1:9".to_string(),
        ohttp_relay_url: None,
        ohttp_delayed: false,
        delayed_wait: crate::config::DelayedWait::default(),
        relay_anchor: None,
        raw: false,
    };
    let key = config.storage_key().unwrap();
    let mut original = Vauchi::new(
        VauchiConfig::with_storage_path(config.storage_path()).with_storage_key(key.clone()),
    )
    .unwrap();
    original.create_identity("Bob original").unwrap();
    let public_id = original.public_id().unwrap();
    drop(original);
    (directory, config, key, public_id)
}

fn backup() -> (String, String) {
    let mut source = Vauchi::in_memory().unwrap();
    source.create_identity("Alice replacement").unwrap();
    (
        source.export_full_backup(PASSWORD).unwrap(),
        source.public_id().unwrap(),
    )
}

// @internal
#[test]
fn full_restore_publication_failure_keeps_the_original_database_and_key() {
    let (_directory, config, key, original_id) = target();
    let (backup, source_id) = backup();
    let marker = config.data_dir.join("keep.txt");
    fs::write(&marker, b"original unrelated file").unwrap();

    let result = restore(&config, &backup, PASSWORD, |prepared, _destination| {
        let prepared_copy =
            Vauchi::new(VauchiConfig::with_storage_path(prepared).with_storage_key(key.clone()))
                .unwrap();
        assert_eq!(prepared_copy.public_id().unwrap(), source_id);
        assert_eq!(
            fs::metadata(prepared.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(prepared).unwrap().permissions().mode() & 0o777,
            0o600
        );
        Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "injected database publication failure",
        ))
    });

    let error = match result {
        Ok(_) => panic!("publication failure must fail the restore"),
        Err(error) => error,
    };
    assert_eq!(error.to_string(), "Cannot publish the restored database");
    assert_eq!(
        error
            .root_cause()
            .downcast_ref::<io::Error>()
            .unwrap()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    let original =
        Vauchi::new(VauchiConfig::with_storage_path(config.storage_path()).with_storage_key(key))
            .unwrap();
    assert_eq!(original.public_id().unwrap(), original_id);
    assert_eq!(original.identity().unwrap().display_name(), "Bob original");
    assert_eq!(fs::read(marker).unwrap(), b"original unrelated file");
}

// @internal
#[test]
fn full_restore_success_reopens_with_the_original_profile_key() {
    let (_directory, config, key, _original_id) = target();
    let (backup, source_id) = backup();
    fs::write(config.identity_path(), b"legacy identity entry").unwrap();
    let saved = config.data_dir.join("saved-full.vauchi");
    fs::write(&saved, backup.as_bytes()).unwrap();

    let restored = restore(&config, &backup, PASSWORD, |from, to| fs::rename(from, to)).unwrap();

    assert_eq!(restored.name, "Alice replacement");
    assert_eq!(restored.public_id, source_id);
    let reopened =
        Vauchi::new(VauchiConfig::with_storage_path(config.storage_path()).with_storage_key(key))
            .unwrap();
    assert_eq!(reopened.public_id().unwrap(), source_id);
    assert_eq!(
        reopened.identity().unwrap().display_name(),
        "Alice replacement"
    );
    assert!(!config.identity_path().exists());
    assert_eq!(fs::read_to_string(saved).unwrap(), backup);
    assert_eq!(
        fs::metadata(config.storage_path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

// @internal
#[test]
fn full_restore_refuses_an_existing_sqlite_connection() {
    let (_directory, config, key, original_id) = target();
    let (backup, _source_id) = backup();
    let original = Vauchi::new(
        VauchiConfig::with_storage_path(config.storage_path()).with_storage_key(key.clone()),
    )
    .unwrap();
    original.storage().wal_checkpoint().unwrap();

    let result = restore(&config, &backup, PASSWORD, |from, to| fs::rename(from, to));

    let error = match result {
        Ok(_) => panic!("an open database must prevent publication"),
        Err(error) => error,
    };
    assert_eq!(
        error.to_string(),
        "The database is not ready for replacement. Close other Vauchi processes and try again."
    );
    assert_eq!(original.public_id().unwrap(), original_id);
    drop(original);
    let reopened =
        Vauchi::new(VauchiConfig::with_storage_path(config.storage_path()).with_storage_key(key))
            .unwrap();
    assert_eq!(reopened.public_id().unwrap(), original_id);
}
