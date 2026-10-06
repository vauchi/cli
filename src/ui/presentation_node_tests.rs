// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Every node kind through the terminal renderer and the prompt
//! (vauchi/private#505).

use std::io::Cursor;

use super::*;
use vauchi_core::{
    AccessibilitySpec, ActionSpec, ActionTone, BindingId, ChoiceOption, Command, Event, InputValue,
    InteractionId, PresentationAxis, PresentationImageShape, PresentationInputKind,
    PresentationListStyle, PresentationNode, PresentationQrPurpose, PresentationRow,
    PresentationTextStyle, PresentationTokens, PresentationTone, SurfaceId, SurfaceLayout,
    SurfaceSpec,
};

fn binding(id: &str) -> BindingId {
    BindingId::new(id).unwrap()
}

fn surface_id(id: &str) -> SurfaceId {
    SurfaceId::new(id).unwrap()
}

fn a11y(label: &str) -> AccessibilitySpec {
    AccessibilitySpec::label(label)
}

fn state_on(id: &str, nodes: Vec<PresentationNode>) -> PresentationState {
    let mut state = PresentationState::default();
    state.apply(&[Command::ReplaceSurface {
        surface: SurfaceSpec {
            surface_id: surface_id(id),
            revision: 1,
            title: "Title".into(),
            subtitle: None,
            accessibility_label: "Title".into(),
            layout: SurfaceLayout::Scroll,
            tokens: PresentationTokens {
                spacing_small: 1,
                spacing_medium: 2,
                spacing_large: 3,
                corner_radius: 1,
                minimum_target_size: 1,
            },
            nodes,
        },
    }]);
    state
}

fn text(content: &str, style: PresentationTextStyle) -> PresentationNode {
    PresentationNode::Text {
        id: None,
        content: content.into(),
        style,
        accessibility: a11y(content),
    }
}

fn input(id: &str, label: &str, value: &str) -> PresentationNode {
    PresentationNode::Input {
        binding_id: binding(id),
        label: label.into(),
        value: value.into(),
        placeholder: Some("type here".into()),
        input_kind: PresentationInputKind::Text,
        max_length: None,
        validation_error: None,
        enabled: true,
        accessibility: a11y(label),
    }
}

fn toggle(id: &str, label: &str, value: bool) -> PresentationNode {
    PresentationNode::Toggle {
        binding_id: binding(id),
        label: label.into(),
        value,
        enabled: true,
        accessibility: a11y(label),
    }
}

fn choice(selected: Option<&str>) -> PresentationNode {
    PresentationNode::Choice {
        binding_id: binding("theme"),
        label: "Theme".into(),
        selected: selected.map(Into::into),
        options: ["light", "dark"]
            .iter()
            .map(|id| ChoiceOption {
                id: (*id).into(),
                label: id.to_uppercase(),
            })
            .collect(),
        enabled: true,
        accessibility: a11y("Theme"),
    }
}

fn slider() -> PresentationNode {
    PresentationNode::Slider {
        binding_id: binding("volume"),
        label: "Volume".into(),
        value: 3.0,
        minimum: 0.0,
        maximum: 10.0,
        step: None,
        minimum_icon: None,
        maximum_icon: None,
        accessibility: a11y("Volume"),
    }
}

fn group(label: Option<&str>, children: Vec<PresentationNode>) -> PresentationNode {
    PresentationNode::Group {
        id: None,
        label: label.map(Into::into),
        axis: PresentationAxis::Vertical,
        children,
        accessibility: a11y("group"),
    }
}

fn list(controls: Vec<PresentationNode>) -> PresentationNode {
    PresentationNode::List {
        style: PresentationListStyle::Rows,
        id: binding("rows"),
        label: Some("Rows".into()),
        rows: vec![PresentationRow {
            title: "Row".into(),
            subtitle: None,
            detail: None,
            icon_token: None,
            image_data: None,
            fallback_text: None,
            selected: false,
            enabled: true,
            activation: None,
            secondary_actions: Vec::new(),
            controls,
            info: None,
            accessibility: a11y("Row"),
        }],
        searchable: false,
        paging: None,
        accessibility: a11y("Rows"),
    }
}

fn qr(id: &str, purpose: PresentationQrPurpose, label: Option<&str>) -> PresentationNode {
    PresentationNode::Qr {
        id: binding(id),
        payloads: vec!["PAYLOAD".into()],
        purpose,
        label: label.map(Into::into),
        placement: None,
        error_correction: None,
        size: None,
        accessibility: a11y("QR"),
    }
}

fn action(id: &str) -> ActionSpec {
    ActionSpec {
        interaction_id: InteractionId::new(id).unwrap(),
        label: id.into(),
        accessibility_label: id.into(),
        icon_token: None,
        enabled: true,
        tone: ActionTone::Standard,
        shortcut: None,
    }
}

fn every_node() -> Vec<PresentationNode> {
    vec![
        text("Plain", PresentationTextStyle::Body),
        input("name", "Name", ""),
        toggle("flag", "Flag", true),
        choice(Some("dark")),
        group(
            Some("Section"),
            vec![text("Nested", PresentationTextStyle::Body)],
        ),
        list(vec![toggle("in-row", "InRow", false)]),
        PresentationNode::Image {
            id: None,
            data: None,
            fallback_text: Some("avatar".into()),
            shape: PresentationImageShape::Circle,
            size: None,
            brightness: 1.0,
            activation: None,
            accessibility: a11y("Avatar"),
        },
        PresentationNode::Status {
            id: None,
            title: "Synced".into(),
            detail: Some("now".into()),
            icon_token: None,
            badge: Some("3".into()),
            tone: PresentationTone::Neutral,
            activation: None,
            accessibility: a11y("Synced"),
        },
        qr("show", PresentationQrPurpose::Display, Some("Mine")),
        qr("scan", PresentationQrPurpose::Capture, Some("Theirs")),
        PresentationNode::Confirmation {
            id: binding("confirm"),
            warning: "Careful".into(),
            confirm: action("yes"),
            cancel: action("no"),
            accessibility: a11y("Confirm"),
        },
        slider(),
        PresentationNode::Progress {
            label: Some("Upload".into()),
            value: Some(0.5),
            accessibility: a11y("Upload"),
        },
        PresentationNode::Divider,
    ]
}

fn lines(rendered: &str) -> Vec<String> {
    console::strip_ansi_codes(rendered)
        .lines()
        .map(str::to_string)
        .collect()
}

// @internal
#[test]
fn every_node_kind_renders_its_line_at_its_depth() {
    let rendered = lines(&render_to_string(&state_on("all", every_node())));

    for expected in [
        "  Plain",
        "  Name: type here",
        "  [x] Flag",
        "  Theme: DARK",
        "  Section",
        "    Nested",
        "  Rows",
        "  Row",
        "    [ ] InRow",
        "  [Image: avatar]",
        "  Synced now 3",
        "  [QR code: Mine]",
        "  [QR input: Theirs]",
        "  Careful",
        "  Volume: 3",
        "  Upload 50%",
        &format!("  {}", "─".repeat(46)),
    ] {
        assert!(
            rendered.iter().any(|line| line == expected),
            "missing {expected:?} in {rendered:#?}"
        );
    }
}

// @internal
#[test]
fn heading_and_muted_text_are_styled_apart_from_body_text() {
    console::set_colors_enabled(true);
    let rendered = render_to_string(&state_on(
        "styles",
        vec![
            text("Heading", PresentationTextStyle::Heading),
            text("Muted", PresentationTextStyle::Muted),
            text("Caption", PresentationTextStyle::Caption),
            text("Body", PresentationTextStyle::Body),
        ],
    ));

    assert!(rendered.contains("\u{1b}[1mHeading"), "{rendered:?}");
    assert!(rendered.contains("\u{1b}[2mMuted"), "{rendered:?}");
    assert!(rendered.contains("\u{1b}[2mCaption"), "{rendered:?}");
    assert!(rendered.contains("  Body\n"), "{rendered:?}");
}

// @internal
#[test]
fn an_unset_or_unknown_choice_renders_a_dash() {
    let unset = lines(&render_to_string(&state_on("c", vec![choice(None)])));
    let unknown = lines(&render_to_string(&state_on(
        "c",
        vec![choice(Some("blue"))],
    )));

    assert!(unset.iter().any(|line| line == "  Theme: —"), "{unset:?}");
    assert!(
        unknown.iter().any(|line| line == "  Theme: —"),
        "{unknown:?}"
    );
}

fn prompt(state: &PresentationState, offered: &mut OfferedInputs, typed: &str) -> (Event, String) {
    let mut input = Cursor::new(typed.as_bytes().to_vec());
    let mut output = Vec::new();
    let event = prompt_event(state, offered, &mut input, &mut output).expect("prompt event");
    (event, String::from_utf8(output).unwrap())
}

fn value_changed(id: &str, binding_id: &str, value: InputValue) -> Event {
    Event::ValueChanged {
        surface_id: surface_id(id),
        binding_id: binding(binding_id),
        value,
    }
}

// @internal
#[test]
fn the_prompt_lists_every_operable_node_and_acts_on_the_chosen_one() {
    let state = state_on(
        "targets",
        vec![
            input("name", "Name", "set"),
            toggle("flag", "Flag", true),
            choice(None),
            slider(),
            group(None, vec![toggle("grouped", "Grouped", false)]),
            list(vec![toggle("listed", "Listed", false)]),
        ],
    );

    let (event, listing) = prompt(&state, &mut OfferedInputs::default(), "2\n");
    assert_eq!(
        event,
        value_changed("targets", "flag", InputValue::Boolean(false))
    );
    for entry in [
        "  1. Edit Name",
        "  2. Flag",
        "  3. Theme: LIGHT",
        "  4. Theme: DARK",
        "  5. Volume",
        "  6. Grouped",
        "  7. Listed",
    ] {
        assert!(listing.contains(entry), "missing {entry:?} in {listing}");
    }

    let choices = [
        (
            "1\nAda\n",
            value_changed("targets", "name", InputValue::Text("Ada".into())),
        ),
        (
            "4\n",
            value_changed("targets", "theme", InputValue::Choice(Some("dark".into()))),
        ),
        (
            "5\nx\n7.5\n",
            value_changed("targets", "volume", InputValue::Number(7.5)),
        ),
        (
            "6\n",
            value_changed("targets", "grouped", InputValue::Boolean(true)),
        ),
        (
            "7\n",
            value_changed("targets", "listed", InputValue::Boolean(true)),
        ),
    ];
    for (typed, expected) in choices {
        assert_eq!(
            prompt(&state, &mut OfferedInputs::default(), typed).0,
            expected,
            "{typed:?}"
        );
    }
}

// @internal
#[test]
fn empty_inputs_are_asked_first_wherever_they_sit() {
    let cases = [
        ("top", vec![input("name", "Name", "")], "name"),
        ("grouped", vec![group(None, vec![input("g", "G", "")])], "g"),
        ("listed", vec![list(vec![input("l", "L", "")])], "l"),
        (
            "qr",
            vec![qr("scan", PresentationQrPurpose::Capture, None)],
            "scan",
        ),
    ];
    for (id, nodes, asked) in cases {
        let (event, output) = prompt(&state_on(id, nodes), &mut OfferedInputs::default(), "v\n");
        assert_eq!(
            event,
            value_changed(id, asked, InputValue::Text("v".into())),
            "{id}"
        );
        assert!(!output.contains("Choose"), "{id}: {output}");
    }

    let (_, output) = prompt(
        &state_on(
            "qr-label",
            vec![qr("scan", PresentationQrPurpose::Capture, None)],
        ),
        &mut OfferedInputs::default(),
        "v\n",
    );
    assert_eq!(output, "  QR data > ");
}

// @internal
#[test]
fn an_empty_input_is_asked_once_per_surface_visit() {
    let first = state_on(
        "form",
        vec![input("name", "Name", ""), toggle("t", "T", false)],
    );
    let other = state_on(
        "other",
        vec![input("name", "Name", ""), toggle("t", "T", false)],
    );
    let mut offered = OfferedInputs::default();

    let (asked, _) = prompt(&first, &mut offered, "\n");
    assert_eq!(
        asked,
        value_changed("form", "name", InputValue::Text(String::new()))
    );

    let (listed, output) = prompt(&first, &mut offered, "2\n");
    assert!(output.contains("  1. Edit Name"), "{output}");
    assert_eq!(
        listed,
        value_changed("form", "t", InputValue::Boolean(true))
    );

    let (asked_again, _) = prompt(&other, &mut offered, "x\n");
    assert_eq!(
        asked_again,
        value_changed("other", "name", InputValue::Text("x".into()))
    );
}

// @internal
#[test]
fn each_nesting_level_indents_two_more_columns() {
    let rendered = lines(&render_to_string(&state_on(
        "deep",
        vec![group(
            None,
            vec![
                group(None, vec![text("Deep", PresentationTextStyle::Body)]),
                list(vec![toggle("t", "Control", false)]),
            ],
        )],
    )));

    assert!(
        rendered.iter().any(|line| line == "      Deep"),
        "{rendered:#?}"
    );
    assert!(
        rendered.iter().any(|line| line == "      [ ] Control"),
        "{rendered:#?}"
    );
}

// @internal
#[test]
fn an_overlay_attaches_only_to_the_current_surface_revision() {
    let mut state = state_on("overlay", Vec::new());
    let overlay = |revision| Command::PresentOverlay {
        surface_id: surface_id("overlay"),
        revision,
        overlay: vauchi_core::OverlaySpec {
            kind: vauchi_core::OverlayKind::ActionMenu,
            title: None,
            body: None,
            close_label: None,
            items: vec![action("item")],
        },
    };

    state.apply(&[overlay(0)]);
    assert!(state.overlay.is_none(), "a stale overlay is dropped");
    state.apply(&[overlay(1)]);
    assert!(
        state.overlay.is_some(),
        "the current revision's overlay is kept"
    );
}

// @internal
#[test]
fn actions_inside_groups_images_statuses_and_confirmations_are_offered() {
    let state = state_on(
        "actions",
        vec![
            group(
                None,
                vec![PresentationNode::Status {
                    id: None,
                    title: "S".into(),
                    detail: None,
                    icon_token: None,
                    badge: None,
                    tone: PresentationTone::Neutral,
                    activation: Some(action("status")),
                    accessibility: a11y("S"),
                }],
            ),
            PresentationNode::Image {
                id: None,
                data: None,
                fallback_text: None,
                shape: PresentationImageShape::Circle,
                size: None,
                brightness: 1.0,
                activation: Some(action("image")),
                accessibility: a11y("I"),
            },
            PresentationNode::Confirmation {
                id: binding("c"),
                warning: "W".into(),
                confirm: action("yes"),
                cancel: action("no"),
                accessibility: a11y("C"),
            },
        ],
    );

    let ids: Vec<String> = state
        .actions()
        .into_iter()
        .map(|a| a.interaction_id.as_str().to_string())
        .collect();

    assert_eq!(ids, ["status", "image", "yes", "no"]);
}
