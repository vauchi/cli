// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for `screen_renderer` — included via `#[path]` so the
//! production module stays under the file-size threshold while
//! tests retain access to private helpers.

use super::*;
use vauchi_app::ui::{DropdownOption, Progress};

// Convenience wrappers used by the snapshot-style tests below.
fn render_text(content: &str, text_style: &TextStyle) {
    let mut out = String::new();
    render_text_to(&mut out, content, text_style);
    print!("{}", out);
}

fn render_actions(actions: &[ScreenAction]) {
    let mut out = String::new();
    render_actions_to(&mut out, actions);
    print!("{}", out);
}

/// `Component::Dropdown` must render as `Label: <selected_label> ▼`,
/// matching the de facto contract that 3 frontends (tui, linux-gtk,
/// android) already enforce: `selected` is a `DropdownOption.id`,
/// the displayed string is the matching option's `label`.
// @internal
#[test]
fn render_dropdown_shows_selected_option_label() {
    console::set_colors_enabled(false);
    let mut out = String::new();
    render_component_to(
        &mut out,
        &Component::Dropdown {
            id: "theme".into(),
            label: "Theme".into(),
            selected: Some("dark".into()),
            options: vec![
                DropdownOption {
                    id: "follow_system".into(),
                    label: "System".into(),
                },
                DropdownOption {
                    id: "dark".into(),
                    label: "Dark".into(),
                },
            ],
            a11y: None,
        },
    );
    assert!(out.contains("Theme"), "expected label, got: {out:?}");
    assert!(
        out.contains("Dark"),
        "expected selected option label, got: {out:?}"
    );
    assert!(out.contains('▼'), "expected caret, got: {out:?}");
}

/// When `selected` is `None` or doesn't match any option id, render
/// the placeholder `—` (mirrors TUI's behavior). Catches the
/// upstream id-vs-label bug honestly instead of silently dropping
/// the component.
// @internal
#[test]
fn render_dropdown_shows_placeholder_when_selected_id_missing() {
    console::set_colors_enabled(false);
    let mut out = String::new();
    render_component_to(
        &mut out,
        &Component::Dropdown {
            id: "language".into(),
            label: "Language".into(),
            selected: Some("xx-unknown".into()),
            options: vec![DropdownOption {
                id: "en".into(),
                label: "English".into(),
            }],
            a11y: None,
        },
    );
    assert!(out.contains("Language"));
    assert!(out.contains('—'), "expected placeholder, got: {out:?}");
}

/// `Component::AvatarPreview` must render something user-visible
/// — historically dropped by the catch-all `_` arm. Placeholder
/// surface acceptable for CLI.
// @internal
#[test]
fn render_avatar_preview_emits_visible_placeholder() {
    console::set_colors_enabled(false);
    let mut out = String::new();
    render_component_to(
        &mut out,
        &Component::AvatarPreview {
            id: "avatar".into(),
            image_data: None,
            initials: "AB".into(),
            bg_color: None,
            brightness: 0.0,
            editable: false,
            a11y: None,
        },
    );
    assert!(
        out.contains("Avatar") || out.contains("AB"),
        "expected avatar surface, got: {out:?}"
    );
}

/// `Component::Slider` must render label + current value — historically
/// dropped by the catch-all `_` arm.
// @internal
#[test]
fn render_slider_emits_label_and_value() {
    console::set_colors_enabled(false);
    let mut out = String::new();
    render_component_to(
        &mut out,
        &Component::Slider {
            id: "brightness".into(),
            label: "Brightness".into(),
            value: 0.5,
            min: 0.0,
            max: 1.0,
            step: 0.1,
            min_icon: None,
            max_icon: None,
            a11y: None,
        },
    );
    assert!(out.contains("Brightness"), "expected label, got: {out:?}");
    assert!(
        out.contains("0.5") || out.contains("0.50"),
        "expected current value, got: {out:?}"
    );
}

// @internal
#[test]
fn render_does_not_panic_on_minimal_screen() {
    let screen = ScreenModel {
        screen_id: "test".into(),
        title: "Test".into(),
        subtitle: None,
        components: vec![],
        actions: vec![],
        progress: None,
        ..Default::default()
    };
    render(&screen);
    assert_eq!(screen.screen_id, "test");
    assert_eq!(screen.title, "Test");
    assert!(screen.subtitle.is_none());
    assert!(screen.components.is_empty());
    assert!(screen.actions.is_empty());
    assert!(screen.progress.is_none());
}

// @internal
#[test]
fn render_does_not_panic_on_all_component_types() {
    let screen = ScreenModel {
        screen_id: "test".into(),
        title: "All Components".into(),
        subtitle: Some("subtitle".into()),
        components: vec![
            Component::Text {
                id: "t".into(),
                content: "Hello".into(),
                style: TextStyle::Body,
            },
            Component::TextInput {
                id: "ti".into(),
                label: "Name".into(),
                value: "Alice".into(),
                placeholder: Some("Enter name".into()),
                max_length: Some(50),
                validation_error: None,
                input_type: vauchi_app::ui::InputType::Text,
                a11y: None,
                info_key: None,
            },
            Component::ToggleList {
                id: "tl".into(),
                label: "Groups".into(),
                items: vec![
                    ToggleItem {
                        id: "a".into(),
                        label: "Family".into(),
                        selected: true,
                        subtitle: None,
                        a11y: None,
                        info_key: None,
                    },
                    ToggleItem {
                        id: "b".into(),
                        label: "Friends".into(),
                        selected: false,
                        subtitle: Some("close friends".into()),
                        a11y: None,
                        info_key: None,
                    },
                ],
                a11y: None,
            },
            Component::FieldList {
                id: "fl".into(),
                fields: vec![FieldDisplay {
                    id: "f0".into(),
                    field_type: "email".into(),
                    label: "work".into(),
                    value: "a@b.com".into(),
                    visibility: UiFieldVisibility::Shown,
                    a11y: None,
                }],
                visibility_mode: VisibilityMode::ShowHide,
                available_groups: vec![],
                a11y: None,
            },
            Component::CardPreview {
                name: "Alice".into(),
                fields: vec![],
                group_views: vec![],
                selected_group: None,
                visible_fields: vec![],
                avatar_data: None,
                a11y: None,
            },
            Component::InfoPanel {
                id: "ip".into(),
                icon: Some("shield".into()),
                title: "Security".into(),
                items: vec![InfoItem {
                    icon: Some("lock".into()),
                    title: "E2E".into(),
                    detail: "Encrypted".into(),
                }],
                a11y: None,
            },
            Component::Divider,
        ],
        actions: vec![
            ScreenAction {
                id: "ok".into(),
                label: "OK".into(),
                style: ActionStyle::Primary,
                enabled: true,
                a11y: None,
            },
            ScreenAction {
                id: "cancel".into(),
                label: "Cancel".into(),
                style: ActionStyle::Destructive,
                enabled: false,
                a11y: None,
            },
        ],
        progress: Some(Progress {
            current_step: 2,
            total_steps: 5,
            label: Some("Name".into()),
        }),
        ..Default::default()
    };
    render(&screen);
    assert_eq!(screen.components.len(), 7);

    // Verify screen-level fields
    assert_eq!(screen.title, "All Components");
    assert_eq!(screen.subtitle.as_deref(), Some("subtitle"));
    assert_eq!(screen.progress.as_ref().unwrap().current_step, 2);
    assert_eq!(screen.progress.as_ref().unwrap().total_steps, 5);
    assert_eq!(
        screen.progress.as_ref().unwrap().label.as_deref(),
        Some("Name")
    );

    // Verify each component's content
    match &screen.components[0] {
        Component::Text {
            id,
            content,
            style: text_style,
            ..
        } => {
            assert_eq!(id, "t");
            assert_eq!(content, "Hello");
            assert!(matches!(text_style, TextStyle::Body));
        }
        other => panic!("Expected Text component, got {:?}", other),
    }

    match &screen.components[1] {
        Component::TextInput {
            id,
            label,
            value,
            placeholder,
            max_length,
            validation_error,
            ..
        } => {
            assert_eq!(id, "ti");
            assert_eq!(label, "Name");
            assert_eq!(value, "Alice");
            assert_eq!(placeholder.as_deref(), Some("Enter name"));
            assert_eq!(*max_length, Some(50));
            assert!(validation_error.is_none());
        }
        other => panic!("Expected TextInput component, got {:?}", other),
    }

    match &screen.components[2] {
        Component::ToggleList {
            id, label, items, ..
        } => {
            assert_eq!(id, "tl");
            assert_eq!(label, "Groups");
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].label, "Family");
            assert!(items[0].selected);
            assert!(items[0].subtitle.is_none());
            assert_eq!(items[1].label, "Friends");
            assert!(!items[1].selected);
            assert_eq!(items[1].subtitle.as_deref(), Some("close friends"));
        }
        other => panic!("Expected ToggleList component, got {:?}", other),
    }

    match &screen.components[3] {
        Component::FieldList {
            id,
            fields,
            visibility_mode,
            available_groups,
            ..
        } => {
            assert_eq!(id, "fl");
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].label, "work");
            assert_eq!(fields[0].value, "a@b.com");
            assert_eq!(fields[0].field_type, "email");
            assert!(matches!(fields[0].visibility, UiFieldVisibility::Shown));
            assert!(matches!(visibility_mode, VisibilityMode::ShowHide));
            assert!(available_groups.is_empty());
        }
        other => panic!("Expected FieldList component, got {:?}", other),
    }

    match &screen.components[4] {
        Component::CardPreview {
            name,
            fields,
            group_views,
            selected_group,
            ..
        } => {
            assert_eq!(name, "Alice");
            assert!(fields.is_empty());
            assert!(group_views.is_empty());
            assert!(selected_group.is_none());
        }
        other => panic!("Expected CardPreview component, got {:?}", other),
    }

    match &screen.components[5] {
        Component::InfoPanel {
            id,
            icon,
            title,
            items,
            ..
        } => {
            assert_eq!(id, "ip");
            assert_eq!(icon.as_deref(), Some("shield"));
            assert_eq!(title, "Security");
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].title, "E2E");
            assert_eq!(items[0].detail, "Encrypted");
            assert_eq!(items[0].icon.as_deref(), Some("lock"));
        }
        other => panic!("Expected InfoPanel component, got {:?}", other),
    }

    assert!(matches!(screen.components[6], Component::Divider));

    // Verify actions
    assert_eq!(screen.actions.len(), 2);
    assert_eq!(screen.actions[0].id, "ok");
    assert_eq!(screen.actions[0].label, "OK");
    assert!(matches!(screen.actions[0].style, ActionStyle::Primary));
    assert!(screen.actions[0].enabled);
    assert_eq!(screen.actions[1].id, "cancel");
    assert_eq!(screen.actions[1].label, "Cancel");
    assert!(matches!(screen.actions[1].style, ActionStyle::Destructive));
    assert!(!screen.actions[1].enabled);
}

// @internal
#[test]
fn render_card_preview_with_group_selection() {
    let screen = ScreenModel {
        screen_id: "preview".into(),
        title: "Preview".into(),
        subtitle: None,
        components: vec![Component::CardPreview {
            name: "Bob".into(),
            fields: vec![FieldDisplay {
                id: "f0".into(),
                field_type: "phone".into(),
                label: "mobile".into(),
                value: "+1234".into(),
                visibility: UiFieldVisibility::Shown,
                a11y: None,
            }],
            group_views: vec![GroupCardView {
                group_name: "Family".into(),
                display_name: "Bob".into(),
                visible_fields: vec![FieldDisplay {
                    id: "f0".into(),
                    field_type: "phone".into(),
                    label: "mobile".into(),
                    value: "+1234".into(),
                    visibility: UiFieldVisibility::Shown,
                    a11y: None,
                }],
            }],
            selected_group: Some("Family".into()),
            visible_fields: vec![FieldDisplay {
                id: "f0".into(),
                field_type: "phone".into(),
                label: "mobile".into(),
                value: "+1234".into(),
                visibility: UiFieldVisibility::Shown,
                a11y: None,
            }],
            avatar_data: None,
            a11y: None,
        }],
        actions: vec![],
        progress: None,
        ..Default::default()
    };
    assert_eq!(screen.components.len(), 1);
    assert_eq!(screen.screen_id, "preview");
    assert_eq!(screen.title, "Preview");

    match &screen.components[0] {
        Component::CardPreview {
            name,
            fields,
            group_views,
            selected_group,
            ..
        } => {
            assert_eq!(name, "Bob");
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].label, "mobile");
            assert_eq!(fields[0].value, "+1234");
            assert_eq!(fields[0].field_type, "phone");
            assert_eq!(selected_group.as_deref(), Some("Family"));
            assert_eq!(group_views.len(), 1);
            assert_eq!(group_views[0].group_name, "Family");
            assert_eq!(group_views[0].display_name, "Bob");
            assert_eq!(group_views[0].visible_fields.len(), 1);
            assert_eq!(group_views[0].visible_fields[0].label, "mobile");
            assert_eq!(group_views[0].visible_fields[0].value, "+1234");
        }
        other => panic!("Expected CardPreview component, got {:?}", other),
    }
}

// @internal
#[test]
fn render_text_styles_do_not_panic() {
    let styles = [
        TextStyle::Title,
        TextStyle::Subtitle,
        TextStyle::Body,
        TextStyle::Caption,
    ];
    for text_style in &styles {
        render_text("content", text_style);
    }
    assert_eq!(styles.len(), 4);
    assert!(matches!(styles[0], TextStyle::Title));
    assert!(matches!(styles[1], TextStyle::Subtitle));
    assert!(matches!(styles[2], TextStyle::Body));
    assert!(matches!(styles[3], TextStyle::Caption));
}

// @internal
#[test]
fn render_actions_all_styles() {
    let actions = vec![
        ScreenAction {
            id: "a".into(),
            label: "Primary".into(),
            style: ActionStyle::Primary,
            enabled: true,
            a11y: None,
        },
        ScreenAction {
            id: "b".into(),
            label: "Secondary".into(),
            style: ActionStyle::Secondary,
            enabled: true,
            a11y: None,
        },
        ScreenAction {
            id: "c".into(),
            label: "Destructive".into(),
            style: ActionStyle::Destructive,
            enabled: true,
            a11y: None,
        },
        ScreenAction {
            id: "d".into(),
            label: "Disabled".into(),
            style: ActionStyle::Primary,
            enabled: false,
            a11y: None,
        },
    ];
    render_actions(&actions);
    assert_eq!(actions.len(), 4);

    assert_eq!(actions[0].id, "a");
    assert_eq!(actions[0].label, "Primary");
    assert!(matches!(actions[0].style, ActionStyle::Primary));
    assert!(actions[0].enabled);

    assert_eq!(actions[1].id, "b");
    assert_eq!(actions[1].label, "Secondary");
    assert!(matches!(actions[1].style, ActionStyle::Secondary));
    assert!(actions[1].enabled);

    assert_eq!(actions[2].id, "c");
    assert_eq!(actions[2].label, "Destructive");
    assert!(matches!(actions[2].style, ActionStyle::Destructive));
    assert!(actions[2].enabled);

    assert_eq!(actions[3].id, "d");
    assert_eq!(actions[3].label, "Disabled");
    assert!(matches!(actions[3].style, ActionStyle::Primary));
    assert!(!actions[3].enabled);
}

// --- Golden fixture snapshot tests ---

/// Helper: load a golden fixture JSON and render to string.
fn render_golden_fixture(fixture_name: &str) -> String {
    // Disable colors so snapshots are deterministic (no ANSI codes)
    console::set_colors_enabled(false);

    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../core/vauchi-core/tests/fixtures/golden")
        .join(fixture_name);
    let json = std::fs::read_to_string(&fixture_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", fixture_path.display(), e));
    let screen: ScreenModel =
        serde_json::from_str(&json).expect("Failed to deserialize golden fixture");
    render_to_string(&screen)
}

// @internal
#[test]
fn golden_snapshot_default_name() {
    let output = render_golden_fixture("default_name.json");
    insta::assert_snapshot!(output);
}

// @internal
#[test]
fn golden_snapshot_groups_setup() {
    let output = render_golden_fixture("groups_setup.json");
    insta::assert_snapshot!(output);
}

// @internal
#[test]
fn golden_snapshot_contact_info() {
    let output = render_golden_fixture("contact_info.json");
    insta::assert_snapshot!(output);
}

// @internal
#[test]
fn golden_snapshot_what_next() {
    let output = render_golden_fixture("what_next.json");
    insta::assert_snapshot!(output);
}
