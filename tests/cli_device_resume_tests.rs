// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

mod common;

use common::{Cli, exchange_data};

fn pending_join() -> (Cli, String) {
    let host = Cli::new();
    host.stdout(&["init", "Alice"]);
    let joiner = Cli::new();
    joiner.stdout(&["init", "Existing"]);
    let qr = exchange_data(&host.stdout(&["device", "link"]));
    let request =
        exchange_data(&joiner.stdout(&["device", "join", &qr, "--device-name", "Second", "--yes"]));
    let response = exchange_data(&host.stdout(&["device", "complete", "--yes", &request]));
    (joiner, response)
}

// @internal
#[test]
fn device_finish_without_usable_pending_key_preserves_existing_identity() {
    for corrupt in [false, true] {
        let (joiner, response) = pending_join();
        let pending = joiner.data_dir().join(".pending_device_join");
        if corrupt {
            std::fs::create_dir_all(&pending).expect("pending directory");
            std::fs::write(pending.join("device_signing.key"), [0_u8; 8]).expect("corrupt key");
        } else {
            match std::fs::remove_dir_all(&pending) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => panic!("remove pending key: {error}"),
            }
        }
        let output = joiner.run(&["device", "finish", &response]);
        assert_eq!(output.status.code(), Some(1));
        assert!(!String::from_utf8_lossy(&output.stderr).contains(&response));
        let card = joiner.run(&["--raw", "card", "show"]);
        assert_eq!(card.status.code(), Some(0), "existing identity was reset");
        let document: serde_json::Value = serde_json::from_slice(&card.stdout).expect("own card");
        assert_eq!(document["display_name"], "Existing");
    }
}

// @internal
#[test]
fn device_finish_removes_the_pending_signing_key_after_adoption() {
    let (joiner, response) = pending_join();
    let output = joiner.run(&["device", "finish", &response]);
    assert_eq!(output.status.code(), Some(0));
    assert!(
        !joiner
            .data_dir()
            .join(".pending_device_join/device_signing.key")
            .exists()
    );
    let card = joiner.run(&["--raw", "card", "show"]);
    assert_eq!(card.status.code(), Some(0));
    let document: serde_json::Value = serde_json::from_slice(&card.stdout).expect("adopted card");
    assert_eq!(document["display_name"], "Alice");
}
