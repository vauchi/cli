// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use vauchi_cli_display::presentation::{LinePrompt, OfferedInputs, PresentationState};
use vauchi_contract::{
    AccessibilitySpec, ActionSpec, ActionTone, BindingId, Command, Event, InputValue,
    InteractionId, OverlayKind, OverlaySpec, PresentationInputKind, PresentationNode, SurfaceSpec,
};

fn state(nodes: Vec<PresentationNode>) -> PresentationState {
    let fixture: serde_json::Value =
        serde_json::from_str(vauchi_contract::fixtures::presentation_contract_v1_json()).unwrap();
    let commands: Vec<Command> =
        serde_json::from_value(fixture["initial_commands"].clone()).unwrap();
    let mut state = PresentationState::default();
    state.apply(&commands);
    let mut surface = state.surface().unwrap().clone();
    surface.nodes = nodes;
    state.apply(&[Command::ReplaceSurface { surface }]);
    state
}

fn input(kind: PresentationInputKind) -> PresentationNode {
    PresentationNode::Input {
        binding_id: BindingId::new("entry").unwrap(),
        label: "Entry".into(),
        value: String::new(),
        placeholder: None,
        input_kind: kind,
        max_length: None,
        validation_error: None,
        enabled: true,
        accessibility: AccessibilitySpec::label("Entry"),
    }
}

fn action() -> ActionSpec {
    ActionSpec {
        interaction_id: InteractionId::new("accept").unwrap(),
        label: "Accept".into(),
        accessibility_label: "Accept".into(),
        icon_token: None,
        enabled: true,
        tone: ActionTone::Standard,
        shortcut: None,
    }
}

// @internal
#[test]
fn line_prompt_secret_preserves_spaces_and_emits_only_once() {
    let state = state(vec![input(PresentationInputKind::Password)]);
    let mut prompt = LinePrompt::new(&state, &mut OfferedInputs::default()).unwrap();
    assert_eq!(prompt.prompt(), "  Entry > ");
    assert!(prompt.is_secret());
    assert_eq!(
        prompt.submit_line(&state, "  secret  ").unwrap(),
        Some(Event::ValueChanged {
            surface_id: state.surface().unwrap().surface_id.clone(),
            binding_id: BindingId::new("entry").unwrap(),
            value: InputValue::Text("  secret  ".into()),
        })
    );
    assert_eq!(
        prompt.submit_line(&state, "again").unwrap_err().kind(),
        std::io::ErrorKind::InvalidInput
    );
}

// @internal
#[test]
fn line_prompt_selection_rejects_zero_and_waits_for_the_selected_field() {
    let state = state(vec![input(PresentationInputKind::Text)]);
    let mut offered = OfferedInputs::default();
    let _ = LinePrompt::new(&state, &mut offered).unwrap();
    let mut prompt = LinePrompt::new(&state, &mut offered).unwrap();
    assert_eq!(prompt.submit_line(&state, "0").unwrap(), None);
    assert!(prompt.prompt().contains("Invalid selection"));
    assert_eq!(prompt.submit_line(&state, "1").unwrap(), None);
    assert_eq!(prompt.prompt(), "  Edit Entry > ");
    assert_eq!(
        prompt.submit_line(&state, "Alice").unwrap(),
        Some(Event::ValueChanged {
            surface_id: state.surface().unwrap().surface_id.clone(),
            binding_id: BindingId::new("entry").unwrap(),
            value: InputValue::Text("Alice".into()),
        })
    );
}

// @internal
#[test]
fn line_prompt_rejects_input_after_the_surface_changes() {
    let mut state = state(vec![input(PresentationInputKind::Text)]);
    let mut prompt = LinePrompt::new(&state, &mut OfferedInputs::default()).unwrap();
    let mut surface: SurfaceSpec = state.surface().unwrap().clone();
    surface.revision += 1;
    state.apply(&[Command::ReplaceSurface { surface }]);
    assert_eq!(
        prompt.submit_line(&state, "stale").unwrap_err().kind(),
        std::io::ErrorKind::InvalidInput
    );
}

// @internal
#[test]
fn line_prompt_without_a_surface_fails_explicitly() {
    let error = LinePrompt::new(&PresentationState::default(), &mut OfferedInputs::default())
        .err()
        .expect("input requires a presentation");
    assert_eq!(error.kind(), std::io::ErrorKind::Other);
}

// @internal
#[test]
fn line_prompt_without_targets_waits_for_a_line_before_back() {
    let state = state(Vec::new());
    let mut prompt = LinePrompt::new(&state, &mut OfferedInputs::default()).unwrap();
    assert_eq!(prompt.prompt(), "  Enter to go back > ");
    assert_eq!(
        prompt.submit_line(&state, "").unwrap(),
        Some(Event::BackRequested {
            surface_id: state.surface().unwrap().surface_id.clone(),
        })
    );
}

// @internal
#[test]
fn line_prompt_overlay_hides_empty_underlying_input_and_rejects_old_prompt() {
    let mut state = state(vec![input(PresentationInputKind::Password)]);
    let mut offered = OfferedInputs::default();
    let mut old_prompt = LinePrompt::new(&state, &mut offered).unwrap();
    state.apply(&[Command::PresentOverlay {
        surface_id: state.surface().unwrap().surface_id.clone(),
        revision: state.surface().unwrap().revision,
        overlay: OverlaySpec {
            kind: OverlayKind::ActionMenu,
            title: None,
            body: None,
            close_label: None,
            items: vec![action()],
        },
    }]);
    assert!(!old_prompt.is_current(&state));
    assert_eq!(
        old_prompt.submit_line(&state, "secret").unwrap_err().kind(),
        std::io::ErrorKind::InvalidInput
    );
    let mut prompt = LinePrompt::new(&state, &mut OfferedInputs::default()).unwrap();
    assert!(!prompt.is_secret());
    assert_eq!(prompt.prompt(), "  1. Accept\n  Choose (1-1) > ");
    assert_eq!(
        prompt.submit_line(&state, "1").unwrap(),
        Some(Event::ActionActivated {
            surface_id: state.surface().unwrap().surface_id.clone(),
            interaction_id: InteractionId::new("accept").unwrap(),
        })
    );
}

// @internal
#[test]
fn line_prompt_number_rejects_nonfinite_and_malformed_input_before_valid_number() {
    let state = state(vec![PresentationNode::Slider {
        binding_id: BindingId::new("number").unwrap(),
        label: "Number".into(),
        value: 0.0,
        minimum: 0.0,
        maximum: 10.0,
        step: None,
        minimum_icon: None,
        maximum_icon: None,
        accessibility: AccessibilitySpec::label("Number"),
    }]);
    let mut prompt = LinePrompt::new(&state, &mut OfferedInputs::default()).unwrap();
    assert_eq!(prompt.submit_line(&state, "1").unwrap(), None);
    for invalid in ["", "NaN", "inf", "-inf", "1e999", "garbage"] {
        assert_eq!(
            prompt.submit_line(&state, invalid).unwrap(),
            None,
            "{invalid}"
        );
        assert_eq!(prompt.prompt(), "  Invalid number. Number > ");
    }
    assert_eq!(
        prompt.submit_line(&state, " 2.5 ").unwrap(),
        Some(Event::ValueChanged {
            surface_id: state.surface().unwrap().surface_id.clone(),
            binding_id: BindingId::new("number").unwrap(),
            value: InputValue::Number(2.5),
        })
    );
}

// @internal
#[test]
fn line_prompt_empty_overlay_dismisses_instead_of_navigating_back() {
    let mut state = state(Vec::new());
    state.apply(&[Command::PresentOverlay {
        surface_id: state.surface().unwrap().surface_id.clone(),
        revision: state.surface().unwrap().revision,
        overlay: OverlaySpec {
            kind: OverlayKind::Information,
            title: None,
            body: Some("Information".into()),
            close_label: Some("Close".into()),
            items: Vec::new(),
        },
    }]);
    let mut prompt = LinePrompt::new(&state, &mut OfferedInputs::default()).unwrap();
    assert_eq!(prompt.prompt(), "  Close > ");
    assert_eq!(
        prompt.submit_line(&state, "").unwrap(),
        Some(Event::OverlayDismissed {
            surface_id: state.surface().unwrap().surface_id.clone(),
            kind: OverlayKind::Information,
        })
    );
}
