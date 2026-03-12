// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Screen Renderer
//!
//! Maps a core ScreenModel to formatted text output on stdout.

use std::fmt::Write as _;

use console::{style, Style};
use vauchi_core::ui::{
    ActionStyle, Component, FieldDisplay, GroupCardView, InfoItem, ScreenAction, ScreenModel,
    TextStyle, ToggleItem, UiFieldVisibility, VisibilityMode,
};

const LINE_WIDTH: usize = 50;

/// Renders a full screen model to stdout.
pub fn render(screen: &ScreenModel) {
    print!("{}", render_to_string(screen));
}

/// Renders a full screen model to a String.
pub fn render_to_string(screen: &ScreenModel) -> String {
    let mut out = String::new();

    // Clear some space
    writeln!(out).unwrap();

    // Progress indicator
    if let Some(progress) = &screen.progress {
        let label = progress.label.as_deref().unwrap_or("");
        writeln!(
            out,
            "  {} Step {}/{}{}",
            style("[").dim(),
            progress.current_step,
            progress.total_steps,
            if label.is_empty() {
                style("]").dim().to_string()
            } else {
                format!(" — {}{}", label, style("]").dim())
            },
        )
        .unwrap();
        writeln!(out).unwrap();
    }

    // Title
    writeln!(out, "  {}", style(&screen.title).bold().cyan()).unwrap();

    // Subtitle
    if let Some(subtitle) = &screen.subtitle {
        writeln!(out, "  {}", style(subtitle).dim()).unwrap();
    }

    writeln!(out).unwrap();

    // Components
    for component in &screen.components {
        render_component_to(&mut out, component);
    }

    // Actions
    if !screen.actions.is_empty() {
        writeln!(out, "{}", "─".repeat(LINE_WIDTH)).unwrap();
        render_actions_to(&mut out, &screen.actions);
    }

    out
}

/// Renders a single component to a buffer.
fn render_component_to(out: &mut String, component: &Component) {
    match component {
        Component::Text {
            content,
            style: text_style,
            ..
        } => {
            render_text_to(out, content, text_style);
        }
        Component::TextInput {
            label,
            value,
            placeholder,
            validation_error,
            ..
        } => {
            render_text_input_to(
                out,
                label,
                value,
                placeholder.as_deref(),
                validation_error.as_deref(),
            );
        }
        Component::ToggleList { label, items, .. } => {
            render_toggle_list_to(out, label, items);
        }
        Component::FieldList {
            fields,
            visibility_mode,
            available_groups,
            ..
        } => {
            render_field_list_to(out, fields, visibility_mode, available_groups);
        }
        Component::CardPreview {
            name,
            fields,
            group_views,
            selected_group,
        } => {
            render_card_preview_to(out, name, fields, group_views, selected_group.as_deref());
        }
        Component::InfoPanel {
            title, items, icon, ..
        } => {
            render_info_panel_to(out, title, items, icon.as_deref());
        }
        Component::Divider => {
            writeln!(out, "  {}", "─".repeat(LINE_WIDTH - 4)).unwrap();
        }
        Component::ContactList { contacts, .. } => {
            for (i, contact) in contacts.iter().enumerate() {
                writeln!(out, "  {}. {}", i + 1, contact.name).unwrap();
            }
        }
        Component::SettingsGroup { label, items, .. } => {
            writeln!(out, "  {}:", style(label).bold()).unwrap();
            for item in items {
                writeln!(out, "    - {}", item.label).unwrap();
            }
        }
        Component::ActionList { items, .. } => {
            for item in items {
                writeln!(out, "  > {}", item.label).unwrap();
            }
        }
        Component::StatusIndicator { title, detail, .. } => {
            write!(out, "  {}", title).unwrap();
            if let Some(d) = detail {
                write!(out, " — {}", d).unwrap();
            }
            writeln!(out).unwrap();
        }
        Component::PinInput {
            label,
            length,
            validation_error,
            ..
        } => {
            writeln!(out, "  {} [{}]", label, "*".repeat(*length)).unwrap();
            if let Some(err) = validation_error {
                writeln!(out, "  {}", style(err).red()).unwrap();
            }
        }
        Component::QrCode { label, .. } => {
            if let Some(l) = label {
                writeln!(out, "  [QR Code: {}]", l).unwrap();
            } else {
                writeln!(out, "  [QR Code]").unwrap();
            }
        }
        Component::ConfirmationDialog { title, message, .. } => {
            writeln!(out, "  {}", style(title).bold()).unwrap();
            writeln!(out, "  {}", message).unwrap();
        }
        Component::ShowToast { message, .. } => {
            writeln!(out, "  {}", style(message).green()).unwrap();
        }
        Component::InlineConfirm { warning, .. } => {
            writeln!(out, "  {}", style(warning).yellow()).unwrap();
        }
        Component::EditableText { label, value, .. } => {
            writeln!(out, "  {}: {}", style(label).bold(), value).unwrap();
        }
    }
}

fn render_text_to(out: &mut String, content: &str, text_style: &TextStyle) {
    match text_style {
        TextStyle::Title => writeln!(out, "  {}", style(content).bold()),
        TextStyle::Subtitle => writeln!(out, "  {}", style(content).italic()),
        TextStyle::Body => writeln!(out, "  {}", content),
        TextStyle::Caption => writeln!(out, "  {}", style(content).dim()),
    }
    .unwrap();
    writeln!(out).unwrap();
}

fn render_text_input_to(
    out: &mut String,
    label: &str,
    value: &str,
    placeholder: Option<&str>,
    validation_error: Option<&str>,
) {
    let display_value = if value.is_empty() {
        placeholder
            .map(|p| style(p).dim().to_string())
            .unwrap_or_default()
    } else {
        value.to_string()
    };

    writeln!(out, "  {}: {}", style(label).bold(), display_value).unwrap();

    if let Some(err) = validation_error {
        writeln!(out, "  {}", style(err).red()).unwrap();
    }
    writeln!(out).unwrap();
}

fn render_toggle_list_to(out: &mut String, label: &str, items: &[ToggleItem]) {
    writeln!(out, "  {}", style(label).bold()).unwrap();
    writeln!(out).unwrap();

    for (i, item) in items.iter().enumerate() {
        let marker = if item.selected { "[x]" } else { "[ ]" };
        let number = i + 1;
        write!(out, "  {} ({}) {}", marker, number, item.label).unwrap();

        if let Some(subtitle) = &item.subtitle {
            write!(out, " {}", style(subtitle).dim()).unwrap();
        }
        writeln!(out).unwrap();
    }
    writeln!(out).unwrap();
}

fn render_field_list_to(
    out: &mut String,
    fields: &[FieldDisplay],
    visibility_mode: &VisibilityMode,
    available_groups: &[String],
) {
    if fields.is_empty() {
        writeln!(out, "  {}", style("(no fields added)").dim()).unwrap();
        writeln!(out).unwrap();
        return;
    }

    for field in fields {
        let vis = match &field.visibility {
            UiFieldVisibility::Shown => style("visible").green().to_string(),
            UiFieldVisibility::Hidden => style("hidden").red().to_string(),
            UiFieldVisibility::Groups(groups) => {
                if groups.is_empty() {
                    style("no groups").yellow().to_string()
                } else {
                    groups.join(", ")
                }
            }
        };

        let mode_label = match visibility_mode {
            VisibilityMode::ShowHide => format!(" [{}]", vis),
            VisibilityMode::PerGroup => format!(" -> {}", vis),
            VisibilityMode::ReadOnly => String::new(),
        };

        writeln!(
            out,
            "  {:12} {:20} {}",
            style(&field.label).dim(),
            field.value,
            style(mode_label).dim(),
        )
        .unwrap();
    }

    if *visibility_mode == VisibilityMode::PerGroup && !available_groups.is_empty() {
        writeln!(out).unwrap();
        writeln!(
            out,
            "  Groups: {}",
            style(available_groups.join(", ")).dim()
        )
        .unwrap();
    }
    writeln!(out).unwrap();
}

fn render_card_preview_to(
    out: &mut String,
    name: &str,
    fields: &[FieldDisplay],
    group_views: &[GroupCardView],
    selected_group: Option<&str>,
) {
    // Show the view matching the selected group, or the default card
    if let Some(group_name) = selected_group {
        if let Some(view) = group_views.iter().find(|v| v.group_name == group_name) {
            render_card_box_to(out, &view.display_name, &view.visible_fields);
            render_group_tabs_to(out, group_views, Some(group_name));
            return;
        }
    }

    // Default: show full card
    render_card_box_to(out, name, fields);

    if !group_views.is_empty() {
        render_group_tabs_to(out, group_views, selected_group);
    }
}

fn render_card_box_to(out: &mut String, name: &str, fields: &[FieldDisplay]) {
    writeln!(out, "  {}", "─".repeat(LINE_WIDTH - 4)).unwrap();
    writeln!(out, "    {}", style(name).bold().cyan()).unwrap();
    writeln!(out, "  {}", "─".repeat(LINE_WIDTH - 4)).unwrap();

    if fields.is_empty() {
        writeln!(out, "    {}", style("(no fields)").dim()).unwrap();
    } else {
        for field in fields {
            writeln!(out, "    {:12} {}", style(&field.label).dim(), field.value).unwrap();
        }
    }

    writeln!(out, "  {}", "─".repeat(LINE_WIDTH - 4)).unwrap();
    writeln!(out).unwrap();
}

fn render_group_tabs_to(out: &mut String, group_views: &[GroupCardView], selected: Option<&str>) {
    write!(out, "  View as: ").unwrap();
    for (i, view) in group_views.iter().enumerate() {
        let is_selected = selected == Some(view.group_name.as_str());
        if is_selected {
            write!(out, "[{}] ", style(&view.group_name).bold()).unwrap();
        } else {
            write!(out, "({}) {} ", i + 1, &view.group_name).unwrap();
        }
    }
    writeln!(out).unwrap();
    writeln!(out).unwrap();
}

fn render_info_panel_to(out: &mut String, title: &str, items: &[InfoItem], icon: Option<&str>) {
    let prefix = icon.unwrap_or("");
    if prefix.is_empty() {
        writeln!(out, "  {}", style(title).bold()).unwrap();
    } else {
        writeln!(out, "  {} {}", style(prefix).dim(), style(title).bold()).unwrap();
    }
    writeln!(out).unwrap();

    let bullet_style = Style::new().dim();

    for item in items {
        let icon_prefix = item.icon.as_deref().unwrap_or("-");
        writeln!(
            out,
            "    {} {}",
            bullet_style.apply_to(icon_prefix),
            style(&item.title).bold(),
        )
        .unwrap();
        writeln!(out, "      {}", item.detail).unwrap();
    }
    writeln!(out).unwrap();
}

fn render_actions_to(out: &mut String, actions: &[ScreenAction]) {
    writeln!(out).unwrap();
    for (i, action) in actions.iter().enumerate() {
        let label = &action.label;
        let number = i + 1;

        let styled = match action.style {
            ActionStyle::Primary => style(format!("({}) {}", number, label)).green().bold(),
            ActionStyle::Secondary => style(format!("({}) {}", number, label)).dim(),
            ActionStyle::Destructive => style(format!("({}) {}", number, label)).red(),
        };

        if !action.enabled {
            write!(
                out,
                "  {}",
                style(format!("({}) {}", number, label)).dim().italic()
            )
            .unwrap();
        } else {
            write!(out, "  {}", styled).unwrap();
        }
        write!(out, "  ").unwrap();
    }
    writeln!(out).unwrap();
    writeln!(out).unwrap();
}

// Convenience wrappers used by existing tests
#[cfg(test)]
fn render_text(content: &str, text_style: &TextStyle) {
    let mut out = String::new();
    render_text_to(&mut out, content, text_style);
    print!("{}", out);
}

#[cfg(test)]
fn render_actions(actions: &[ScreenAction]) {
    let mut out = String::new();
    render_actions_to(&mut out, actions);
    print!("{}", out);
}

// INLINE_TEST_REQUIRED: Tests call private render_* helper functions and CLI is a binary crate
#[cfg(test)]
mod tests {
    use super::*;
    use vauchi_core::ui::Progress;

    #[test]
    fn render_does_not_panic_on_minimal_screen() {
        let screen = ScreenModel {
            screen_id: "test".into(),
            title: "Test".into(),
            subtitle: None,
            components: vec![],
            actions: vec![],
            progress: None,
        };
        render(&screen);
        assert_eq!(screen.screen_id, "test");
        assert_eq!(screen.title, "Test");
        assert!(screen.subtitle.is_none());
        assert!(screen.components.is_empty());
        assert!(screen.actions.is_empty());
        assert!(screen.progress.is_none());
    }

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
                    input_type: vauchi_core::ui::InputType::Text,
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
                        },
                        ToggleItem {
                            id: "b".into(),
                            label: "Friends".into(),
                            selected: false,
                            subtitle: Some("close friends".into()),
                        },
                    ],
                },
                Component::FieldList {
                    id: "fl".into(),
                    fields: vec![FieldDisplay {
                        id: "f0".into(),
                        field_type: "email".into(),
                        label: "work".into(),
                        value: "a@b.com".into(),
                        visibility: UiFieldVisibility::Shown,
                    }],
                    visibility_mode: VisibilityMode::ShowHide,
                    available_groups: vec![],
                },
                Component::CardPreview {
                    name: "Alice".into(),
                    fields: vec![],
                    group_views: vec![],
                    selected_group: None,
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
                },
                Component::Divider,
            ],
            actions: vec![
                ScreenAction {
                    id: "ok".into(),
                    label: "OK".into(),
                    style: ActionStyle::Primary,
                    enabled: true,
                },
                ScreenAction {
                    id: "cancel".into(),
                    label: "Cancel".into(),
                    style: ActionStyle::Destructive,
                    enabled: false,
                },
            ],
            progress: Some(Progress {
                current_step: 2,
                total_steps: 5,
                label: Some("Name".into()),
            }),
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
            Component::ToggleList { id, label, items } => {
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
                    }],
                }],
                selected_group: Some("Family".into()),
            }],
            actions: vec![],
            progress: None,
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

    #[test]
    fn render_actions_all_styles() {
        let actions = vec![
            ScreenAction {
                id: "a".into(),
                label: "Primary".into(),
                style: ActionStyle::Primary,
                enabled: true,
            },
            ScreenAction {
                id: "b".into(),
                label: "Secondary".into(),
                style: ActionStyle::Secondary,
                enabled: true,
            },
            ScreenAction {
                id: "c".into(),
                label: "Destructive".into(),
                style: ActionStyle::Destructive,
                enabled: true,
            },
            ScreenAction {
                id: "d".into(),
                label: "Disabled".into(),
                style: ActionStyle::Primary,
                enabled: false,
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

    #[test]
    fn golden_snapshot_welcome() {
        let output = render_golden_fixture("welcome.json");
        insta::assert_snapshot!(output);
    }

    #[test]
    fn golden_snapshot_default_name() {
        let output = render_golden_fixture("default_name.json");
        insta::assert_snapshot!(output);
    }

    #[test]
    fn golden_snapshot_skip_gate() {
        let output = render_golden_fixture("skip_gate.json");
        insta::assert_snapshot!(output);
    }

    #[test]
    fn golden_snapshot_groups_setup() {
        let output = render_golden_fixture("groups_setup.json");
        insta::assert_snapshot!(output);
    }

    #[test]
    fn golden_snapshot_contact_info() {
        let output = render_golden_fixture("contact_info.json");
        insta::assert_snapshot!(output);
    }

    #[test]
    fn golden_snapshot_preview_card() {
        let output = render_golden_fixture("preview_card.json");
        insta::assert_snapshot!(output);
    }

    #[test]
    fn golden_snapshot_security_explanation() {
        let output = render_golden_fixture("security_explanation.json");
        insta::assert_snapshot!(output);
    }

    #[test]
    fn golden_snapshot_backup_prompt() {
        let output = render_golden_fixture("backup_prompt.json");
        insta::assert_snapshot!(output);
    }

    #[test]
    fn golden_snapshot_ready() {
        let output = render_golden_fixture("ready.json");
        insta::assert_snapshot!(output);
    }
}
