// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt::Write as _;

use console::style;
use vauchi_core::{
    PresentationNode, PresentationQrPurpose, PresentationRow, PresentationTextStyle,
};

use super::PresentationState;

pub fn render_to_string(state: &PresentationState) -> String {
    let Some(surface) = state.surface() else {
        return String::new();
    };
    let mut out = String::new();
    let _ = writeln!(out);
    let _ = writeln!(out, "  {}", style(&surface.title).bold().cyan());
    if let Some(subtitle) = &surface.subtitle {
        let _ = writeln!(out, "  {}", style(subtitle).dim());
    }
    let _ = writeln!(out);
    for node in &surface.nodes {
        render_node(&mut out, node, 2);
    }
    let actions = state.actions();
    if !actions.is_empty() {
        let _ = writeln!(out, "{}", "─".repeat(50));
        for (index, action) in actions.iter().enumerate() {
            let disabled = if action.enabled { "" } else { " (disabled)" };
            let _ = writeln!(out, "  {}. {}{}", index + 1, action.label, disabled);
        }
    }
    out
}

fn render_node(out: &mut String, node: &PresentationNode, indent: usize) {
    let pad = " ".repeat(indent);
    match node {
        PresentationNode::Text {
            content,
            style: text_style,
            ..
        } => match text_style {
            PresentationTextStyle::Heading => {
                let _ = writeln!(out, "{pad}{}", style(content).bold());
            }
            PresentationTextStyle::Muted | PresentationTextStyle::Caption => {
                let _ = writeln!(out, "{pad}{}", style(content).dim());
            }
            _ => {
                let _ = writeln!(out, "{pad}{content}");
            }
        },
        PresentationNode::Input {
            label,
            value,
            placeholder,
            validation_error,
            ..
        } => {
            let shown = if value.is_empty() {
                placeholder.as_deref().unwrap_or("—")
            } else {
                value
            };
            let _ = writeln!(out, "{pad}{}: {shown}", style(label).bold());
            if let Some(error) = validation_error {
                let _ = writeln!(out, "{pad}{}", style(error).red());
            }
        }
        PresentationNode::Toggle { label, value, .. } => {
            let mark = if *value { "x" } else { " " };
            let _ = writeln!(out, "{pad}[{mark}] {label}");
        }
        PresentationNode::Choice {
            label,
            selected,
            options,
            ..
        } => {
            let selected_label = selected
                .as_ref()
                .and_then(|id| options.iter().find(|option| &option.id == id))
                .map(|option| option.label.as_str())
                .unwrap_or("—");
            let _ = writeln!(out, "{pad}{}: {selected_label}", style(label).bold());
        }
        PresentationNode::Group {
            label, children, ..
        } => {
            if let Some(label) = label {
                let _ = writeln!(out, "{pad}{}", style(label).bold());
            }
            for child in children {
                render_node(out, child, indent + 2);
            }
        }
        PresentationNode::List { label, rows, .. } => {
            if let Some(label) = label {
                let _ = writeln!(out, "{pad}{}", style(label).bold());
            }
            for row in rows {
                render_row(out, row, indent);
            }
        }
        PresentationNode::Image { fallback_text, .. } => {
            let _ = writeln!(
                out,
                "{pad}[Image: {}]",
                fallback_text.as_deref().unwrap_or("image")
            );
        }
        PresentationNode::Status {
            title,
            detail,
            badge,
            ..
        } => {
            let detail = detail.as_deref().unwrap_or("");
            let badge = badge.as_deref().unwrap_or("");
            let _ = writeln!(out, "{pad}{title} {detail} {badge}");
        }
        PresentationNode::Qr { purpose, label, .. } => {
            let kind = match purpose {
                PresentationQrPurpose::Display => "QR code",
                PresentationQrPurpose::Capture => "QR input",
                _ => "QR",
            };
            let _ = writeln!(out, "{pad}[{kind}: {}]", label.as_deref().unwrap_or(""));
        }
        PresentationNode::Confirmation { warning, .. } => {
            let _ = writeln!(out, "{pad}{}", style(warning).yellow());
        }
        PresentationNode::Slider { label, value, .. } => {
            let _ = writeln!(out, "{pad}{label}: {value}");
        }
        PresentationNode::Progress { label, value, .. } => {
            let percent = value.map(|value| format!(" {:.0}%", value * 100.0));
            let _ = writeln!(
                out,
                "{pad}{}{}",
                label.as_deref().unwrap_or("Progress"),
                percent.as_deref().unwrap_or("")
            );
        }
        PresentationNode::Divider => {
            let _ = writeln!(out, "{pad}{}", "─".repeat(46));
        }
        _ => {
            let _ = writeln!(out, "{pad}[Unsupported presentation element]");
        }
    }
}

fn render_row(out: &mut String, row: &PresentationRow, indent: usize) {
    let pad = " ".repeat(indent);
    let selected = if row.selected { "✓ " } else { "" };
    let _ = writeln!(out, "{pad}{selected}{}", row.title);
    if let Some(subtitle) = &row.subtitle {
        let _ = writeln!(out, "{pad}  {}", style(subtitle).dim());
    }
    for control in &row.controls {
        render_node(out, control, indent + 2);
    }
}
