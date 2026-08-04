// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;
use serde::Deserialize;
use vauchi_core::{
    AccessibilitySpec, ActionSpec, ActionTone, BindingId, Command, ContextBar, Event,
    FilePickPurpose, InputValue, InteractionId, PresentationInputKind, PresentationNode,
    PresentationTextStyle, PresentationTokens, StandardShortcut, SurfaceId, SurfaceLayout,
    SurfaceSpec,
};

// Fixture versions are exact contracts: additive fields require an explicit
// consumer review rather than being ignored silently.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PresentationContractFixture {
    schema_version: u64,
    initial_commands: Vec<Command>,
    steps: Vec<PresentationContractStep>,
    expected_state: ExpectedPresentationState,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PresentationContractStep {
    // The CLI replays Core's commands, but decoding the event still verifies
    // that this consumer agrees with the shell-to-Core wire shape.
    #[serde(rename = "event")]
    _event: Event,
    commands: Vec<Command>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedPresentationState {
    active_surface_id: SurfaceId,
    surface: SurfaceSpec,
    context_bar: ContextBar,
}

fn action(id: &str, label: &str) -> ActionSpec {
    ActionSpec {
        interaction_id: InteractionId::new(id).unwrap(),
        label: label.into(),
        accessibility_label: label.into(),
        icon_token: None,
        enabled: true,
        tone: ActionTone::Standard,
        shortcut: Some(StandardShortcut::ActivatePrimary),
    }
}

fn surface(revision: u64, title: &str) -> SurfaceSpec {
    SurfaceSpec {
        surface_id: SurfaceId::new("welcome").unwrap(),
        revision,
        title: title.into(),
        subtitle: Some("Prepared by Core".into()),
        accessibility_label: title.into(),
        layout: SurfaceLayout::Scroll,
        tokens: PresentationTokens {
            spacing_small: 1,
            spacing_medium: 2,
            spacing_large: 3,
            corner_radius: 0,
            minimum_target_size: 1,
        },
        nodes: vec![PresentationNode::Text {
            id: None,
            content: "Hello".into(),
            style: PresentationTextStyle::Body,
            accessibility: AccessibilitySpec::label("Hello"),
        }],
    }
}

// @scenario: generic_presentation_protocol.feature :: Every shell renders the same prepared presentation
#[test]
fn command_batch_atomically_installs_surface_and_context_bar() {
    let mut state = PresentationState::default();
    state.apply(&[
        Command::ReplaceSurface {
            surface: surface(4, "Welcome"),
        },
        Command::SetContextBar {
            surface_id: SurfaceId::new("welcome").unwrap(),
            revision: 4,
            bar: Box::new(ContextBar {
                primary: Some(action("surface.4.primary", "Continue")),
                ..ContextBar::default()
            }),
        },
    ]);

    assert_eq!(state.surface().unwrap().revision, 4);
    assert_eq!(
        state.context_bar().unwrap().primary.as_ref().unwrap().label,
        "Continue"
    );
}

// @scenario: generic_presentation_protocol.feature :: Invalid boundary input fails safely
#[test]
fn older_surface_replacement_cannot_roll_back_render_state() {
    let mut state = PresentationState::default();
    state.apply(&[
        Command::ReplaceSurface {
            surface: surface(5, "Newest"),
        },
        Command::ReplaceSurface {
            surface: surface(4, "Stale"),
        },
    ]);

    assert_eq!(state.surface().unwrap().title, "Newest");
}

// @scenario: generic_presentation_protocol.feature :: Invalid boundary input fails safely
#[test]
fn older_context_bar_cannot_attach_to_a_newer_surface_revision() {
    let mut state = PresentationState::default();
    state.apply(&[
        Command::ReplaceSurface {
            surface: surface(5, "Newest"),
        },
        Command::SetContextBar {
            surface_id: SurfaceId::new("welcome").unwrap(),
            revision: 5,
            bar: Box::new(ContextBar {
                primary: Some(action("new", "New action")),
                ..ContextBar::default()
            }),
        },
        Command::SetContextBar {
            surface_id: SurfaceId::new("welcome").unwrap(),
            revision: 4,
            bar: Box::new(ContextBar {
                primary: Some(action("old", "Stale action")),
                ..ContextBar::default()
            }),
        },
    ]);

    assert_eq!(
        state.context_bar().unwrap().primary.as_ref().unwrap().label,
        "New action"
    );
}

// @scenario: generic_presentation_protocol.feature :: Every shell renders the same prepared presentation
#[test]
fn renderer_and_activation_use_only_generic_protocol_values() {
    let mut state = PresentationState::default();
    state.apply(&[
        Command::ReplaceSurface {
            surface: surface(2, "Welcome"),
        },
        Command::SetContextBar {
            surface_id: SurfaceId::new("welcome").unwrap(),
            revision: 2,
            bar: Box::new(ContextBar {
                primary: Some(action("opaque-token", "Continue")),
                ..ContextBar::default()
            }),
        },
    ]);

    let rendered = render_to_string(&state);
    assert!(rendered.contains("Welcome"));
    assert!(rendered.contains("Hello"));
    assert!(rendered.contains("Continue"));
    assert_eq!(
        state.activation(0),
        Some(vauchi_core::Event::ActionActivated {
            surface_id: SurfaceId::new("welcome").unwrap(),
            interaction_id: InteractionId::new("opaque-token").unwrap(),
        })
    );
}

// @scenario: generic_presentation_protocol.feature :: Every shell renders the same prepared presentation
#[test]
fn cli_consumes_the_core_owned_presentation_contract_fixture() {
    let fixture: PresentationContractFixture =
        serde_json::from_str(vauchi_app::ui::presentation_contract_fixture_json())
            .expect("failed to deserialize Core-owned presentation contract fixture");
    let mut state = PresentationState::default();

    assert_eq!(
        fixture.schema_version, 1,
        "fixture schema changed; re-verify the CLI reducer contract"
    );
    assert!(!fixture.initial_commands.is_empty());
    assert!(!fixture.steps.is_empty());
    assert_eq!(
        fixture.expected_state.surface.surface_id,
        fixture.expected_state.active_surface_id
    );

    let effects = state.apply(&fixture.initial_commands);
    assert!(
        effects.is_empty(),
        "initial fixture batch emitted shell effects: {effects:?}"
    );
    for (index, step) in fixture.steps.into_iter().enumerate() {
        assert!(!step.commands.is_empty(), "fixture step {index} is empty");
        let effects = state.apply(&step.commands);
        assert!(
            effects.is_empty(),
            "fixture step {index} emitted shell effects: {effects:?}"
        );
    }

    assert_eq!(
        state.surface().map(|surface| surface.surface_id.as_str()),
        Some(fixture.expected_state.active_surface_id.as_str())
    );
    assert_eq!(state.surface(), Some(&fixture.expected_state.surface));
    assert_eq!(
        state.context_bar(),
        Some(&fixture.expected_state.context_bar)
    );
}

// @scenario: generic_presentation_protocol.feature :: User interaction returns as an opaque event
#[test]
fn prompt_returns_raw_value_for_core_minted_binding() {
    let mut state = PresentationState::default();
    let mut input_surface = surface(3, "Name");
    input_surface.nodes = vec![PresentationNode::Input {
        binding_id: BindingId::new("opaque-binding").unwrap(),
        label: "Display name".into(),
        value: String::new(),
        placeholder: None,
        input_kind: PresentationInputKind::Text,
        max_length: Some(80),
        validation_error: None,
        enabled: true,
        accessibility: AccessibilitySpec::label("Display name"),
    }];
    state.apply(&[Command::ReplaceSurface {
        surface: input_surface,
    }]);
    let mut input = std::io::Cursor::new(b"Alice\n");
    let mut output = Vec::new();

    let event = prompt_event(&state, &mut input, &mut output).expect("prompt event");

    assert_eq!(
        event,
        Event::ValueChanged {
            surface_id: SurfaceId::new("welcome").unwrap(),
            binding_id: BindingId::new("opaque-binding").unwrap(),
            value: InputValue::Text("Alice".into()),
        }
    );
}

struct FakeReducer {
    initial: Vec<Command>,
}

impl CommandReducer for FakeReducer {
    type Error = std::convert::Infallible;

    fn initial_commands(&mut self) -> Result<Vec<Command>, Self::Error> {
        Ok(std::mem::take(&mut self.initial))
    }

    fn dispatch(&mut self, event: Event) -> Result<Vec<Command>, Self::Error> {
        assert!(matches!(
            event,
            Event::ActionActivated { interaction_id, .. }
                if interaction_id.as_str() == "opaque-token"
        ));
        Ok(vec![Command::PerformNativeBack])
    }
}

// @internal
#[test]
fn generic_session_exits_only_when_core_requests_native_back() {
    let mut reducer = FakeReducer {
        initial: vec![
            Command::ReplaceSurface {
                surface: surface(1, "Session"),
            },
            Command::SetContextBar {
                surface_id: SurfaceId::new("welcome").unwrap(),
                revision: 1,
                bar: Box::new(ContextBar {
                    primary: Some(action("opaque-token", "Continue")),
                    ..ContextBar::default()
                }),
            },
        ],
    };
    let mut input = std::io::Cursor::new(b"1\n");
    let mut output = Vec::new();

    run_with_io(&mut reducer, &mut input, &mut output).expect("generic session");

    assert!(String::from_utf8(output).unwrap().contains("Session"));
}

struct FilePickReducer {
    expected_bytes: Vec<u8>,
    expected_filename: String,
}

impl CommandReducer for FilePickReducer {
    type Error = std::convert::Infallible;

    fn initial_commands(&mut self) -> Result<Vec<Command>, Self::Error> {
        Ok(vec![Command::FilePickFromUser {
            accepted_mime_types: vec!["application/octet-stream".into()],
            purpose: FilePickPurpose::ImportBackup,
        }])
    }

    fn dispatch(&mut self, event: Event) -> Result<Vec<Command>, Self::Error> {
        assert_eq!(
            event,
            Event::FilePickedFromUser {
                bytes: self.expected_bytes.clone(),
                filename: self.expected_filename.clone(),
            }
        );
        Ok(vec![Command::PerformNativeBack])
    }
}

// @internal
#[test]
fn generic_session_adapts_a_core_file_pick_to_terminal_input() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("backup.vauchi");
    let bytes = b"encrypted backup".to_vec();
    std::fs::write(&path, &bytes).expect("test backup");
    let mut reducer = FilePickReducer {
        expected_bytes: bytes,
        expected_filename: "backup.vauchi".into(),
    };
    let mut input = std::io::Cursor::new(format!("{}\n", path.display()));
    let mut output = Vec::new();

    run_with_io(&mut reducer, &mut input, &mut output).expect("file-pick session");

    assert!(
        String::from_utf8(output)
            .expect("utf-8 terminal output")
            .contains("File path")
    );
}
