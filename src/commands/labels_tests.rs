// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Label ages and the avatar input ceiling (vauchi/private#505).

use std::fs::File;

use tempfile::TempDir;

use super::{MAX_AVATAR_INPUT_BYTES, format_age, format_timestamp, read_avatar_file};

// @internal
#[test]
fn ages_change_unit_at_each_boundary() {
    let cases = [
        (59, "just now"),
        (60, "1 minutes ago"),
        (3_599, "59 minutes ago"),
        (3_600, "1 hours ago"),
        (86_399, "23 hours ago"),
        (86_400, "1 days ago"),
        (3 * 86_400, "3 days ago"),
    ];
    for (elapsed, phrase) in cases {
        assert_eq!(format_age(elapsed), phrase, "{elapsed}");
    }
}

// @internal
#[test]
fn a_timestamp_is_aged_against_the_cli_clock() {
    let now = crate::clock::unix_seconds();

    assert_eq!(format_timestamp(now - 2 * 3_600 - 5), "2 hours ago");
    assert_eq!(format_timestamp(now + 1_000), "just now");
}

fn file_of(len: u64) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("avatar.png");
    File::create(&path).unwrap().set_len(len).unwrap();
    (dir, path)
}

// @internal
#[test]
fn the_avatar_ceiling_is_eight_mebibytes_inclusive() {
    assert_eq!(MAX_AVATAR_INPUT_BYTES, 8 * 1024 * 1024);

    let (_dir, at_limit) = file_of(MAX_AVATAR_INPUT_BYTES);
    assert_eq!(
        read_avatar_file(&at_limit).unwrap().len() as u64,
        MAX_AVATAR_INPUT_BYTES
    );

    let (_dir, over) = file_of(MAX_AVATAR_INPUT_BYTES + 1);
    let error = read_avatar_file(&over).unwrap_err().to_string();
    assert!(error.contains("the limit is 8388608 bytes"), "{error}");

    let (_dir, empty) = file_of(0);
    assert!(
        read_avatar_file(&empty)
            .unwrap_err()
            .to_string()
            .contains("is empty")
    );
}
