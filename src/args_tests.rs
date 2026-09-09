// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Clap parsing tests for the crate-private `Cli`/`Commands` surface.
//! Sibling of `args.rs` (the `#[path]` module there) because these types
//! are not reachable from `tests/` without making the arg surface `pub`.

use clap::{CommandFactory, Parser};

use super::*;

// @internal
#[test]
fn ohttp_relay_flag_parses_when_provided() {
    let cli = Cli::parse_from([
        "vauchi",
        "--ohttp-relay",
        "https://ohttp.self.example",
        "sync",
    ]);
    assert_eq!(
        cli.ohttp_relay.as_deref(),
        Some("https://ohttp.self.example")
    );
}

// @internal
#[test]
fn ohttp_relay_defaults_to_none() {
    // Unset: core derives the OHTTP endpoint from the relay URL.
    let cli = Cli::parse_from(["vauchi", "sync"]);
    assert_eq!(cli.ohttp_relay, None);
}

// @internal
#[test]
fn backup_password_flag_parses_for_export_and_import() {
    let export = Cli::parse_from(["vauchi", "export", "out.vauchi", "--password", "hunter2"]);
    let Commands::Export { password, .. } = export.command else {
        panic!("expected export");
    };
    assert_eq!(password.as_deref(), Some("hunter2"));

    let import = Cli::parse_from(["vauchi", "import", "in.vauchi", "--password", "hunter2"]);
    let Commands::Import { password, .. } = import.command else {
        panic!("expected import");
    };
    assert_eq!(password.as_deref(), Some("hunter2"));
}

// @internal
#[test]
fn import_yes_flag_parses_and_defaults_to_false() {
    let bare = Cli::parse_from(["vauchi", "import", "in.vauchi"]);
    let Commands::Import { yes, .. } = bare.command else {
        panic!("expected import");
    };
    assert!(!yes, "unset must keep the interactive confirmation");

    let flagged = Cli::parse_from(["vauchi", "import", "in.vauchi", "--yes"]);
    let Commands::Import { yes, .. } = flagged.command else {
        panic!("expected import");
    };
    assert!(yes);
}

// @internal
#[test]
fn backup_password_defaults_to_none_so_the_prompt_still_runs() {
    // Unset must stay None — a default would silently skip the interactive
    // confirmation and encrypt a backup under a password nobody chose.
    let cli = Cli::parse_from(["vauchi", "export", "out.vauchi"]);
    let Commands::Export { password, .. } = cli.command else {
        panic!("expected export");
    };
    assert_eq!(password, None);
}

// @internal
#[test]
fn labels_set_name_parses_value_or_clear_flag() {
    let set = Cli::parse_from(["vauchi", "labels", "set-name", "Business", "Dr. Egloff"]);
    let Commands::Labels(LabelCommands::SetName { label, name, clear }) = set.command else {
        panic!("expected labels set-name");
    };
    assert_eq!(label, "Business");
    assert_eq!(name.as_deref(), Some("Dr. Egloff"));
    assert!(!clear);

    let cleared = Cli::parse_from(["vauchi", "labels", "set-name", "Business", "--clear"]);
    let Commands::Labels(LabelCommands::SetName { name, clear, .. }) = cleared.command else {
        panic!("expected labels set-name");
    };
    assert_eq!(name, None);
    assert!(clear);
}

// @internal
#[test]
fn labels_set_name_rejects_missing_value_and_value_with_clear() {
    assert!(Cli::try_parse_from(["vauchi", "labels", "set-name", "Business"]).is_err());
    assert!(
        Cli::try_parse_from(["vauchi", "labels", "set-name", "Business", "x", "--clear"]).is_err()
    );
}

// @internal
#[test]
fn labels_set_bio_and_set_avatar_parse_value_or_clear_flag() {
    let bio = Cli::parse_from(["vauchi", "labels", "set-bio", "Business", "Consultant"]);
    let Commands::Labels(LabelCommands::SetBio { label, bio, clear }) = bio.command else {
        panic!("expected labels set-bio");
    };
    assert_eq!(label, "Business");
    assert_eq!(bio.as_deref(), Some("Consultant"));
    assert!(!clear);

    let avatar = Cli::parse_from(["vauchi", "labels", "set-avatar", "Business", "me.png"]);
    let Commands::Labels(LabelCommands::SetAvatar { label, path, clear }) = avatar.command else {
        panic!("expected labels set-avatar");
    };
    assert_eq!(label, "Business");
    assert_eq!(path.as_deref(), Some(std::path::Path::new("me.png")));
    assert!(!clear);

    let cleared = Cli::parse_from(["vauchi", "labels", "set-avatar", "Business", "--clear"]);
    let Commands::Labels(LabelCommands::SetAvatar { path, clear, .. }) = cleared.command else {
        panic!("expected labels set-avatar");
    };
    assert_eq!(path, None);
    assert!(clear);
    assert!(Cli::try_parse_from(["vauchi", "labels", "set-bio", "Business"]).is_err());
    assert!(Cli::try_parse_from(["vauchi", "labels", "set-avatar", "Business"]).is_err());
}

// @internal
#[test]
fn contacts_clear_override_parses_contact_and_field() {
    let cli = Cli::parse_from(["vauchi", "contacts", "clear-override", "Bob Jones", "Work"]);
    let Commands::Contacts(ContactCommands::ClearOverride { contact, field }) = cli.command else {
        panic!("expected contacts clear-override");
    };
    assert_eq!(contact, "Bob Jones");
    assert_eq!(field, "Work");
}

// @internal
#[test]
fn cli_command_definition_is_valid() {
    // allow(zero_assertions): debug_assert() validates the clap command
    // graph (it panics on conflicting/misconfigured args); not a
    // recognised assertion macro.
    Cli::command().debug_assert();
}
