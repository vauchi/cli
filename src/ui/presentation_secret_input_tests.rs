// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Core sends the typed app password as a password input's value and leaves
//! masking to the shell (lock screen; the password form since #621), so a
//! terminal that prints values as-is shows the password to whoever reads it
//! (vauchi/private#619).

use super::*;
use vauchi_core::{
    AccessibilitySpec, BindingId, Command, PresentationInputKind, PresentationNode,
    PresentationTokens, SurfaceId, SurfaceLayout, SurfaceSpec,
};

fn rendered_input(input_kind: PresentationInputKind, value: &str) -> String {
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
    console::strip_ansi_codes(&render_to_string(&state)).into_owned()
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
