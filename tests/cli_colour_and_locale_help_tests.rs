// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! `NO_COLOR` turns colour off, and `--locale`'s help names every shipped
//! locale, through the binary (vauchi/private#746, ship checklist SC-58).

mod common;

use common::Cli;

const ESCAPE: &str = "\u{1b}[";

/// `CLICOLOR_FORCE` makes the colour crate style a pipe, so the tests can
/// see colour without a terminal.
const FORCED: (&str, &str) = ("CLICOLOR_FORCE", "1");

fn activity_output(envs: &[(&str, &str)]) -> String {
    let cli = Cli::new();
    cli.stdout(&["init", "Alice"]);
    cli.stdout_env(&["activity"], envs)
}

// @internal
#[test]
fn forced_colour_reaches_a_pipe() {
    let output = activity_output(&[FORCED]);

    assert!(
        output.contains(ESCAPE),
        "expected styled output: {output:?}"
    );
}

// @internal
#[test]
fn no_color_turns_colour_off_even_when_forced() {
    let output = activity_output(&[FORCED, ("NO_COLOR", "1")]);

    assert!(
        !output.contains(ESCAPE),
        "escape codes under NO_COLOR: {output:?}"
    );
    assert!(output.contains("Recent Activity"), "{output:?}");
}

// @internal
#[test]
fn an_empty_no_color_changes_nothing() {
    let output = activity_output(&[FORCED, ("NO_COLOR", "")]);

    assert!(
        output.contains(ESCAPE),
        "an empty NO_COLOR is not set: {output:?}"
    );
}

// @internal
#[test]
fn the_locale_help_names_every_shipped_locale() {
    let help = Cli::new().stdout(&["--help"]);
    let line = help
        .lines()
        .find(|line| line.contains("Locale for output messages"))
        .unwrap_or_else(|| panic!("no locale line in help: {help}"));

    for code in ["en", "de", "fr", "es", "it"] {
        assert!(
            line.split(|c: char| !c.is_ascii_alphabetic())
                .any(|word| word == code),
            "the locale help does not name {code}: {line}"
        );
    }
}
