// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Screen Renderer
//!
//! Maps a core ScreenModel to formatted text output on stdout.

use std::fmt::Write as _;

use console::{Style, style};
use vauchi_app::ui::{
    ActionStyle, Component, Field, InfoItem, PreviewVariant, ScreenAction, ScreenModel, TextStyle,
    ToggleItem, UiFieldVisibility, VisibilityMode,
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
        Component::Preview {
            name,
            fields,
            variants,
            selected_variant,
            ..
        } => {
            render_card_preview_to(out, name, fields, variants, selected_variant.as_deref());
        }
        Component::InfoPanel {
            title, items, icon, ..
        } => {
            render_info_panel_to(out, title, items, icon.as_deref());
        }
        Component::Divider => {
            writeln!(out, "  {}", "─".repeat(LINE_WIDTH - 4)).unwrap();
        }
        Component::List { items, .. } => {
            for (i, item) in items.iter().enumerate() {
                writeln!(out, "  {}. {}", i + 1, item.name).unwrap();
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
        Component::InlineConfirm { warning, .. } => {
            writeln!(out, "  {}", style(warning).yellow()).unwrap();
        }
        Component::EditableText { label, value, .. } => {
            writeln!(out, "  {}: {}", style(label).bold(), value).unwrap();
        }
        Component::Banner {
            text, action_label, ..
        } => {
            writeln!(out, "  {} [{}]", style(text).cyan(), action_label).unwrap();
        }
        Component::Dropdown {
            label,
            selected,
            options,
            ..
        } => {
            render_dropdown_to(out, label, selected.as_deref(), options);
        }
        Component::AvatarPreview { initials, .. } => {
            writeln!(out, "  [Avatar: {}]", initials).unwrap();
        }
        Component::Slider { label, value, .. } => {
            writeln!(out, "  {}: {:.2}", style(label).bold(), value).unwrap();
        }
        _ => {
            // Future Component variants — caught by CC-22 reachability
            // test (F5 in 2026-05-03-tui-cli-next-work-audit.md).
        }
    }
}

const DROPDOWN_NONE_PLACEHOLDER: &str = "—";

/// Render a `Component::Dropdown` as `Label: <selected_label> ▼`.
/// `selected` is matched against `options[i].id` (the de facto contract
/// shared with tui/linux-gtk/android). When the id has no match, the
/// `—` placeholder makes upstream id-vs-label bugs visible instead of
/// silently dropping the row.
fn render_dropdown_to(
    out: &mut String,
    label: &str,
    selected: Option<&str>,
    options: &[vauchi_app::ui::DropdownOption],
) {
    let selected_label = selected
        .and_then(|sel_id| options.iter().find(|o| o.id == sel_id))
        .map(|o| o.label.as_str())
        .unwrap_or(DROPDOWN_NONE_PLACEHOLDER);
    writeln!(out, "  {}: {} ▼", style(label).bold(), selected_label).unwrap();
}

fn render_text_to(out: &mut String, content: &str, text_style: &TextStyle) {
    match text_style {
        TextStyle::Title => writeln!(out, "  {}", style(content).bold()),
        TextStyle::Subtitle => writeln!(out, "  {}", style(content).italic()),
        TextStyle::Body => writeln!(out, "  {}", content),
        TextStyle::Caption => writeln!(out, "  {}", style(content).dim()),
        _ => writeln!(out, "  {}", content),
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
    fields: &[Field],
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
            _ => style("unknown").dim().to_string(),
        };

        let mode_label = match visibility_mode {
            VisibilityMode::ShowHide => format!(" [{}]", vis),
            VisibilityMode::PerGroup => format!(" -> {}", vis),
            VisibilityMode::ReadOnly => String::new(),
            _ => String::new(),
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
    fields: &[Field],
    variants: &[PreviewVariant],
    selected_variant: Option<&str>,
) {
    // Show the view matching the selected variant, or the default card
    if let Some(variant_id) = selected_variant
        && let Some(view) = variants.iter().find(|v| v.variant_id == variant_id)
    {
        render_card_box_to(out, &view.display_name, &view.visible_fields);
        render_group_tabs_to(out, variants, Some(variant_id));
        return;
    }

    // Default: show full card
    render_card_box_to(out, name, fields);

    if !variants.is_empty() {
        render_group_tabs_to(out, variants, selected_variant);
    }
}

fn render_card_box_to(out: &mut String, name: &str, fields: &[Field]) {
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

fn render_group_tabs_to(out: &mut String, variants: &[PreviewVariant], selected: Option<&str>) {
    write!(out, "  View as: ").unwrap();
    for (i, view) in variants.iter().enumerate() {
        let is_selected = selected == Some(view.variant_id.as_str());
        if is_selected {
            write!(out, "[{}] ", style(&view.variant_id).bold()).unwrap();
        } else {
            write!(out, "({}) {} ", i + 1, &view.variant_id).unwrap();
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
            _ => style(format!("({}) {}", number, label)),
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

// INLINE_TEST_REQUIRED: Tests call private render_* helper functions; CLI is a binary crate.
// Tests live in a sibling file so this module stays under the file-size threshold.
#[cfg(test)]
#[path = "screen_renderer_tests.rs"]
mod tests;
