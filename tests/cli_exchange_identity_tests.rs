// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Exchange setup must not apply human backup-password rules to install secrets (#1247).

mod common;

use common::{Cli, exchange_data};
use std::fs;
use std::net::TcpListener;

fn synthetic_installation_secret() -> String {
    let mut bytes: [u8; 32] =
        std::array::from_fn(|index| (index as u8).wrapping_mul(73).wrapping_add(29));
    bytes[..3].copy_from_slice(&[0x12, 0x34, 0x56]);
    hex::encode(bytes)
}

fn set_installation_secret(cli: &Cli) -> String {
    let secret = synthetic_installation_secret();
    let path = cli.data_dir().join(".backup-password");
    fs::write(&path, &secret).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    secret
}

// @scenario: contact_exchange :: Generate exchange QR code
#[test]
fn test_qr_start_accepts_generated_installation_secret() {
    let alice = Cli::new();
    alice.stdout(&["init", "Alice"]);
    let secret = set_installation_secret(&alice);
    let alice_qr = exchange_data(&alice.stdout(&["exchange", "start"]));
    let bob = Cli::new();
    bob.stdout(&["init", "Bob"]);
    let bob_qr = exchange_data(&bob.stdout(&["exchange", "start"]));

    alice.stdout(&["exchange", "complete", &bob_qr]);
    bob.stdout(&["exchange", "complete", &alice_qr]);

    assert!(alice.stdout(&["contacts", "list"]).contains("Bob"));
    assert!(bob.stdout(&["contacts", "list"]).contains("Alice"));
    assert_eq!(
        fs::read_to_string(alice.data_dir().join(".backup-password")).unwrap(),
        secret
    );
}

// @scenario: contact_exchange :: Mutual QR exchange with bidirectional scanning
#[test]
fn test_qr_resume_accepts_generated_installation_secret() {
    let alice = Cli::new();
    alice.stdout(&["init", "Alice"]);
    let alice_qr = exchange_data(&alice.stdout(&["exchange", "start"]));
    let bob = Cli::new();
    bob.stdout(&["init", "Bob"]);
    let bob_qr = exchange_data(&bob.stdout(&["exchange", "start"]));
    let secret = set_installation_secret(&alice);

    alice.stdout(&["exchange", "complete", &bob_qr]);
    bob.stdout(&["exchange", "complete", &alice_qr]);

    assert!(alice.stdout(&["contacts", "list"]).contains("Bob"));
    assert!(bob.stdout(&["contacts", "list"]).contains("Alice"));
    assert_eq!(
        fs::read_to_string(alice.data_dir().join(".backup-password")).unwrap(),
        secret
    );
}

// @internal
#[test]
fn test_usb_initiator_reaches_address_validation_with_generated_installation_secret() {
    let alice = Cli::new();
    alice.stdout(&["init", "Alice"]);
    set_installation_secret(&alice);

    let output = alice.run(&["exchange", "usb", "--address", "invalid-address"]);

    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(
        error.contains("invalid address 'invalid-address'"),
        "{error}"
    );
}

// @internal
#[test]
fn test_usb_responder_reaches_bind_error_with_generated_installation_secret() {
    let alice = Cli::new();
    alice.stdout(&["init", "Alice"]);
    set_installation_secret(&alice);
    let occupied = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = occupied.local_addr().unwrap().port().to_string();

    let output = alice.run(&["exchange", "usb-listen", "--port", &port]);

    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("bind failed:"), "{error}");
}

// @scenario: identity_management :: Backup password requirements
#[test]
fn test_user_backup_still_rejects_blocklisted_password() {
    let alice = Cli::new();
    alice.stdout(&["init", "Alice"]);
    let secret = synthetic_installation_secret();
    let backup = alice.data_dir().join("rejected.vauchi");

    let output = alice.run_with_env(
        &["export", backup.to_str().unwrap(), "--identity-only"],
        &[("VAUCHI_BACKUP_PASSWORD", secret.as_str())],
    );

    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("Password too weak"), "{error}");
    assert!(!error.contains(&secret));
    assert!(!backup.exists());
}
