// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Social recovery end to end through the binary: a claim, vouchers from
//! the old identity's contacts, the assembled proof, and how strangers
//! rate it (vauchi/private#505).

mod common;

use common::{Cli, exchange_data};

fn identity(name: &str) -> (Cli, String) {
    let cli = Cli::new();
    let output = cli.stdout(&["init", name]);
    let public_id = output
        .lines()
        .find_map(|line| line.trim().strip_prefix("Public ID: "))
        .unwrap_or_else(|| panic!("no Public ID in {output}"))
        .to_string();
    (cli, public_id)
}

fn exchange(a: &Cli, b: &Cli) {
    let a_data = exchange_data(&a.stdout(&["exchange", "start"]));
    let b_data = exchange_data(&b.stdout(&["exchange", "start"]));
    b.stdout(&["exchange", "complete", &a_data]);
    a.stdout(&["exchange", "complete", &b_data]);
}

/// The base64 blob a recovery command prints for sharing.
fn blob(output: &str) -> String {
    exchange_data(output)
}

fn stderr(cli: &Cli, args: &[&str]) -> String {
    let output = cli.run(args);
    assert!(!output.status.success(), "{args:?} should fail");
    String::from_utf8_lossy(&output.stderr).to_string()
}

struct World {
    old_id: String,
    new_alice: Cli,
    bob: Cli,
    carol: Cli,
    dan: Cli,
}

/// Old Alice exchanged with Bob, Carol and Dan, then lost her device;
/// new Alice has a fresh identity.
fn world() -> World {
    let (old_alice, old_id) = identity("Alice");
    let (bob, _) = identity("Bob");
    let (carol, _) = identity("Carol");
    let (dan, _) = identity("Dan");
    for friend in [&bob, &carol, &dan] {
        exchange(&old_alice, friend);
    }
    let (new_alice, _) = identity("Alice New");
    World {
        old_id,
        new_alice,
        bob,
        carol,
        dan,
    }
}

fn vouch(voucher: &Cli, claim: &str) -> String {
    let output = voucher.stdout(&["recovery", "vouch", claim, "--yes"]);
    assert!(
        output.contains("Auto-confirmed vouch for: Alice"),
        "the claim's old key is this voucher's contact: {output}"
    );
    blob(&output[output.find("Recovery Voucher Created").expect("voucher")..])
}

// @internal
#[test]
fn a_claim_needs_a_32_byte_key_that_is_not_ones_own() {
    let (cli, own_id) = identity("Alice");

    assert!(stderr(&cli, &["recovery", "claim", "abcd"]).contains("Invalid public key"));
    assert!(
        stderr(&cli, &["recovery", "claim", &own_id])
            .contains("Cannot create recovery claim for your own current key")
    );
}

// @internal
#[test]
fn a_full_round_builds_a_proof_that_strangers_rate_by_mutual_contacts() {
    let w = world();
    let claim = blob(&w.new_alice.stdout(&["recovery", "claim", &w.old_id]));

    let first = vouch(&w.bob, &claim);
    w.new_alice.stdout(&["recovery", "add-voucher", &first]);
    assert!(
        stderr(&w.new_alice, &["recovery", "proof"]).contains("Recovery proof incomplete"),
        "one voucher is not a proof"
    );
    for friend in [&w.carol, &w.dan] {
        let voucher = vouch(friend, &claim);
        w.new_alice.stdout(&["recovery", "add-voucher", &voucher]);
    }
    let proof_output = w.new_alice.stdout(&["recovery", "proof"]);
    assert!(proof_output.contains("Vouchers:     3"), "{proof_output}");
    let proof = blob(&proof_output);

    // Strangers to old Alice: Eve knows Bob and Carol, Frank knows Bob,
    // Gina knows nobody involved.
    let (eve, _) = identity("Eve");
    exchange(&eve, &w.bob);
    exchange(&eve, &w.carol);
    let (frank, _) = identity("Frank");
    exchange(&frank, &w.bob);
    let (gina, _) = identity("Gina");

    let high = eve.stdout(&["recovery", "verify", &proof]);
    assert!(high.contains("HIGH CONFIDENCE"), "{high}");
    assert!(
        high.contains("Old identity is NOT in your contacts."),
        "{high}"
    );
    let medium = frank.stdout(&["recovery", "verify", &proof]);
    assert!(medium.contains("MEDIUM CONFIDENCE"), "{medium}");
    let low = gina.stdout(&["recovery", "verify", &proof]);
    assert!(low.contains("LOW CONFIDENCE"), "{low}");
}

// @internal
#[test]
fn a_voucher_must_be_signed_and_match_the_recovery_in_progress() {
    let w = world();
    let claim = blob(&w.new_alice.stdout(&["recovery", "claim", &w.old_id]));
    let voucher = vouch(&w.bob, &claim);
    w.new_alice.stdout(&["recovery", "add-voucher", &voucher]);

    let mut tampered = voucher.clone().into_bytes();
    let middle = tampered.len() / 2;
    tampered[middle] = if tampered[middle] == b'A' { b'B' } else { b'A' };
    let tampered = String::from_utf8(tampered).unwrap();
    let rejected = w.new_alice.run(&["recovery", "add-voucher", &tampered]);
    assert!(!rejected.status.success(), "a tampered voucher is refused");

    // Same new identity, different old key.
    let (_, bob_id) = identity("Somebody");
    let other_old = blob(&w.new_alice.stdout(&["recovery", "claim", &bob_id]));
    let other_old_voucher = w.carol.stdout(&["recovery", "vouch", &other_old, "--yes"]);
    let other_old_voucher = blob(
        &other_old_voucher[other_old_voucher
            .find("Recovery Voucher Created")
            .expect("voucher")..],
    );
    assert!(
        stderr(
            &w.new_alice,
            &["recovery", "add-voucher", &other_old_voucher]
        )
        .contains("Voucher keys don't match the recovery in progress")
    );

    // Same old key, different new identity.
    let (other_new, _) = identity("Alice Other");
    let other_new_claim = blob(&other_new.stdout(&["recovery", "claim", &w.old_id]));
    let other_new_voucher = vouch(&w.dan, &other_new_claim);
    assert!(
        stderr(
            &w.new_alice,
            &["recovery", "add-voucher", &other_new_voucher]
        )
        .contains("Voucher keys don't match the recovery in progress")
    );
}

// @internal
#[test]
fn settings_warn_when_too_few_contacts_are_trusted() {
    let (cli, _) = identity("Alice");

    let output = cli.stdout(&["recovery", "settings", "show"]);

    assert!(
        output.contains("You have fewer trusted contacts (0) than the recovery threshold"),
        "{output}"
    );
}
