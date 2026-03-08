// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Screen Renderer
//!
//! Maps a core ScreenModel to formatted text output on stdout.

use console::{style, Style};
use vauchi_core::ui::{
    ActionStyle, Component, FieldDisplay, GroupCardView, InfoItem, ScreenAction, ScreenModel,
    TextStyle, ToggleItem, UiFieldVisibility, VisibilityMode,
};

const LINE_WIDTH: usize = 50;

/// Renders a full screen model to stdout.
pub fn render(screen: &ScreenModel) {
    // Clear some space
    println!();

    // Progress indicator
    if let Some(progress) = &screen.progress {
        let label = progress.label.as_deref().unwrap_or("");
        println!(
            "  {} Step {}/{}{}",
            style("[").dim(),
            progress.current_step,
            progress.total_steps,
            if label.is_empty() {
                style("]").dim().to_string()
            } else {
                format!(" — {}{}", label, style("]").dim())
            },
        );
        println!();
    }

    // Title
    println!("  {}", style(&screen.title).bold().cyan());

    // Subtitle
    if let Some(subtitle) = &screen.subtitle {
        println!("  {}", style(subtitle).dim());
    }

    println!();

    // Components
    for component in &screen.components {
        render_component(component);
    }

    // Actions
    if !screen.actions.is_empty() {
        println!("{}", "─".repeat(LINE_WIDTH));
        render_actions(&screen.actions);
    }
}

/// Renders a single component.
fn render_component(component: &Component) {
    match component {
        Component::Text {
            content,
            style: text_style,
            ..
        } => {
            render_text(content, text_style);
        }
        Component::TextInput {
            label,
            value,
            placeholder,
            validation_error,
            ..
        } => {
            render_text_input(
                label,
                value,
                placeholder.as_deref(),
                validation_error.as_deref(),
            );
        }
        Component::ToggleList { label, items, .. } => {
            render_toggle_list(label, items);
        }
        Component::FieldList {
            fields,
            visibility_mode,
            available_groups,
            ..
        } => {
            render_field_list(fields, visibility_mode, available_groups);
        }
        Component::CardPreview {
            name,
            fields,
            group_views,
            selected_group,
        } => {
            render_card_preview(name, fields, group_views, selected_group.as_deref());
        }
        Component::InfoPanel {
            title, items, icon, ..
        } => {
            render_info_panel(title, items, icon.as_deref());
        }
        Component::Divider => {
            println!("  {}", "─".repeat(LINE_WIDTH - 4));
        }
    }
}

fn render_text(content: &str, text_style: &TextStyle) {
    match text_style {
        TextStyle::Title => println!("  {}", style(content).bold()),
        TextStyle::Subtitle => println!("  {}", style(content).italic()),
        TextStyle::Body => println!("  {}", content),
        TextStyle::Caption => println!("  {}", style(content).dim()),
    }
    println!();
}

fn render_text_input(
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

    println!("  {}: {}", style(label).bold(), display_value);

    if let Some(err) = validation_error {
        println!("  {}", style(err).red());
    }
    println!();
}

fn render_toggle_list(label: &str, items: &[ToggleItem]) {
    println!("  {}", style(label).bold());
    println!();

    for (i, item) in items.iter().enumerate() {
        let marker = if item.selected { "[x]" } else { "[ ]" };
        let number = i + 1;
        print!("  {} ({}) {}", marker, number, item.label);

        if let Some(subtitle) = &item.subtitle {
            print!(" {}", style(subtitle).dim());
        }
        println!();
    }
    println!();
}

fn render_field_list(
    fields: &[FieldDisplay],
    visibility_mode: &VisibilityMode,
    available_groups: &[String],
) {
    if fields.is_empty() {
        println!("  {}", style("(no fields added)").dim());
        println!();
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
        };

        println!(
            "  {:12} {:20} {}",
            style(&field.label).dim(),
            field.value,
            style(mode_label).dim(),
        );
    }

    if *visibility_mode == VisibilityMode::PerGroup && !available_groups.is_empty() {
        println!();
        println!("  Groups: {}", style(available_groups.join(", ")).dim());
    }
    println!();
}

fn render_card_preview(
    name: &str,
    fields: &[FieldDisplay],
    group_views: &[GroupCardView],
    selected_group: Option<&str>,
) {
    // Show the view matching the selected group, or the default card
    if let Some(group_name) = selected_group {
        if let Some(view) = group_views.iter().find(|v| v.group_name == group_name) {
            render_card_box(&view.display_name, &view.visible_fields);
            render_group_tabs(group_views, Some(group_name));
            return;
        }
    }

    // Default: show full card
    render_card_box(name, fields);

    if !group_views.is_empty() {
        render_group_tabs(group_views, selected_group);
    }
}

fn render_card_box(name: &str, fields: &[FieldDisplay]) {
    println!("  {}", "─".repeat(LINE_WIDTH - 4));
    println!("    {}", style(name).bold().cyan());
    println!("  {}", "─".repeat(LINE_WIDTH - 4));

    if fields.is_empty() {
        println!("    {}", style("(no fields)").dim());
    } else {
        for field in fields {
            println!("    {:12} {}", style(&field.label).dim(), field.value);
        }
    }

    println!("  {}", "─".repeat(LINE_WIDTH - 4));
    println!();
}

fn render_group_tabs(group_views: &[GroupCardView], selected: Option<&str>) {
    print!("  View as: ");
    for (i, view) in group_views.iter().enumerate() {
        let is_selected = selected == Some(view.group_name.as_str());
        if is_selected {
            print!("[{}] ", style(&view.group_name).bold());
        } else {
            print!("({}) {} ", i + 1, &view.group_name);
        }
    }
    println!();
    println!();
}

fn render_info_panel(title: &str, items: &[InfoItem], icon: Option<&str>) {
    let prefix = icon.unwrap_or("");
    if prefix.is_empty() {
        println!("  {}", style(title).bold());
    } else {
        println!("  {} {}", style(prefix).dim(), style(title).bold());
    }
    println!();

    let bullet_style = Style::new().dim();

    for item in items {
        let icon_prefix = item.icon.as_deref().unwrap_or("-");
        println!(
            "    {} {}",
            bullet_style.apply_to(icon_prefix),
            style(&item.title).bold(),
        );
        println!("      {}", item.detail);
    }
    println!();
}

fn render_actions(actions: &[ScreenAction]) {
    println!();
    for (i, action) in actions.iter().enumerate() {
        let label = &action.label;
        let number = i + 1;

        let styled = match action.style {
            ActionStyle::Primary => style(format!("({}) {}", number, label)).green().bold(),
            ActionStyle::Secondary => style(format!("({}) {}", number, label)).dim(),
            ActionStyle::Destructive => style(format!("({}) {}", number, label)).red(),
        };

        if !action.enabled {
            print!(
                "  {}",
                style(format!("({}) {}", number, label)).dim().italic()
            );
        } else {
            print!("  {}", styled);
        }
        print!("  ");
    }
    println!();
    println!();
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
}
