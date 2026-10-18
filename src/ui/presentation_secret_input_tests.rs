// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Core sends the typed app password as a password input's value and leaves
//! masking to the shell (lock screen; the password form since #621), so a
//! terminal that prints values as-is shows the password to whoever reads it
//! (vauchi/private#619). Typing it is the shell's job too: read as an ordinary
//! line, the terminal echoes the password as it is entered
//! (vauchi/private#630).

use std::io::{self, Cursor};

use super::*;
use vauchi_core::{
    AccessibilitySpec, BindingId, Command, Event, InputValue, PresentationInputKind,
    PresentationNode, PresentationTokens, SurfaceId, SurfaceLayout, SurfaceSpec,
};

fn state_with_input(input_kind: PresentationInputKind, value: &str) -> PresentationState {
    let mut state = PresentationState::default();
    state.apply(&[Command::ReplaceSurface {
        surface: SurfaceSpec {
            surface_id: SurfaceId::new("lock").unwrap(),
            revision: 1,
            title: "Vauchi is Locked".into(),
            subtitle: None,
            accessibility_label: "Vauchi is Locked".into(),
            layout: SurfaceLayout::Scroll,
            tokens: PresentationTokens {
                spacing_small: 1,
                spacing_medium: 2,
                spacing_large: 3,
                corner_radius: 1,
                minimum_target_size: 1,
            },
            nodes: vec![PresentationNode::Input {
                binding_id: BindingId::new("surface.1.pin").unwrap(),
                label: "Password".into(),
                value: value.into(),
                placeholder: None,
                input_kind,
                max_length: Some(128),
                validation_error: None,
                enabled: true,
                accessibility: AccessibilitySpec::label("Password entry"),
            }],
        },
    }]);
    state
}

fn rendered_input(input_kind: PresentationInputKind, value: &str) -> String {
    console::strip_ansi_codes(&render_to_string(&state_with_input(input_kind, value))).into_owned()
}

// @internal
#[test]
fn a_password_input_prints_one_bullet_per_character_not_the_password() {
    let rendered = rendered_input(PresentationInputKind::Password, "pass1234");

    assert!(!rendered.contains("pass1234"), "{rendered}");
    assert!(rendered.contains("Password: ••••••••"), "{rendered}");
}

// @internal
#[test]
fn a_pin_input_prints_bullets_not_the_digits() {
    let rendered = rendered_input(PresentationInputKind::Pin, "654321");

    assert!(!rendered.contains("654321"), "{rendered}");
    assert!(rendered.contains("Password: ••••••"), "{rendered}");
}

// @internal
#[test]
fn a_text_input_still_prints_its_value() {
    let rendered = rendered_input(PresentationInputKind::Text, "Alice");

    assert!(rendered.contains("Password: Alice"), "{rendered}");
}

/// Stands in for the terminal's no-echo prompt and records what it asked.
#[derive(Default)]
struct HiddenEntry {
    asked: Vec<String>,
}

impl SecretReader for HiddenEntry {
    fn read_secret(&mut self, label: &str) -> io::Result<String> {
        self.asked.push(label.to_string());
        Ok("s3cret".into())
    }
}

fn typed(state: &PresentationState, stdin: &str, secrets: &mut HiddenEntry) -> Event {
    let mut input = Cursor::new(stdin.as_bytes().to_vec());
    let mut output = Vec::new();
    prompt_event_with(
        state,
        &mut OfferedInputs::default(),
        &mut input,
        &mut output,
        secrets,
    )
    .expect("prompt event")
}

fn entered(value: &str) -> Event {
    Event::ValueChanged {
        surface_id: SurfaceId::new("lock").unwrap(),
        binding_id: BindingId::new("surface.1.pin").unwrap(),
        value: InputValue::Text(value.into()),
    }
}

// @internal
#[test]
fn a_password_input_is_typed_through_the_hidden_prompt_not_the_echoing_line() {
    let mut secrets = HiddenEntry::default();

    let event = typed(
        &state_with_input(PresentationInputKind::Password, ""),
        "visible-on-screen\n",
        &mut secrets,
    );

    assert_eq!(event, entered("s3cret"));
    assert_eq!(secrets.asked, vec!["Password".to_string()]);
}

// @internal
#[test]
fn a_pin_input_is_typed_through_the_hidden_prompt() {
    let mut secrets = HiddenEntry::default();

    let event = typed(
        &state_with_input(PresentationInputKind::Pin, ""),
        "123456\n",
        &mut secrets,
    );

    assert_eq!(event, entered("s3cret"));
    assert_eq!(secrets.asked, vec!["Password".to_string()]);
}

// @internal
#[test]
fn a_text_input_is_typed_on_the_line_and_never_asks_the_hidden_prompt() {
    let mut secrets = HiddenEntry::default();

    let event = typed(
        &state_with_input(PresentationInputKind::Text, ""),
        "Alice\n",
        &mut secrets,
    );

    assert_eq!(event, entered("Alice"));
    assert!(secrets.asked.is_empty());
}

// @internal
#[test]
fn scripted_runs_without_a_terminal_still_read_a_password_from_input() {
    let mut input = Cursor::new(b"pass1234\n".to_vec());
    let mut output = Vec::new();

    let event = prompt_event(
        &state_with_input(PresentationInputKind::Password, ""),
        &mut OfferedInputs::default(),
        &mut input,
        &mut output,
    )
    .expect("prompt event");

    assert_eq!(event, entered("pass1234"));
}
