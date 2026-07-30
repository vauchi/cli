// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{self, BufRead, Write};

use vauchi_core::{
    BindingId, Event, InputValue, PresentationNode, PresentationQrPurpose, StandardShortcut,
    SurfaceId,
};

use super::PresentationState;

pub fn prompt_event(
    state: &PresentationState,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> io::Result<Event> {
    let surface = state
        .surface()
        .ok_or_else(|| io::Error::other("Core has not presented a surface"))?;
    let surface_id = surface.surface_id.clone();
    let mut targets = Vec::new();
    if state.overlay.is_none() {
        for node in &surface.nodes {
            collect_targets(node, &surface_id, &mut targets);
        }
    }
    targets.extend(state.actions().into_iter().map(|action| Target::Ready {
        label: action.label,
        event: Event::ActionActivated {
            surface_id: surface_id.clone(),
            interaction_id: action.interaction_id,
        },
    }));

    if let Some(target) = first_empty_input(&surface.nodes) {
        return read_target(target, surface_id, input, output);
    }
    if targets.is_empty() {
        return Ok(Event::BackRequested { surface_id });
    }
    for (index, target) in targets.iter().enumerate() {
        writeln!(output, "  {}. {}", index + 1, target.label())?;
    }
    write!(output, "  Choose (1-{}) > ", targets.len())?;
    output.flush()?;
    loop {
        let selected = read_line(input)?;
        if selected.is_empty()
            && let Some(event) = default_primary(state)
        {
            return Ok(event);
        }
        if let Ok(index) = selected.parse::<usize>()
            && let Some(target) = targets.get(index.saturating_sub(1))
        {
            return read_target(target.clone(), surface_id, input, output);
        }
        write!(output, "  Invalid selection > ")?;
        output.flush()?;
    }
}

#[derive(Clone)]
enum Target {
    Ready {
        label: String,
        event: Event,
    },
    Text {
        label: String,
        binding_id: BindingId,
    },
    Number {
        label: String,
        binding_id: BindingId,
    },
}

impl Target {
    fn label(&self) -> &str {
        match self {
            Self::Ready { label, .. } | Self::Text { label, .. } | Self::Number { label, .. } => {
                label
            }
        }
    }
}

fn read_target(
    target: Target,
    surface_id: SurfaceId,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> io::Result<Event> {
    match target {
        Target::Ready { event, .. } => Ok(event),
        Target::Text { label, binding_id } => {
            write!(output, "  {label} > ")?;
            output.flush()?;
            Ok(Event::ValueChanged {
                surface_id,
                binding_id,
                value: InputValue::Text(read_line(input)?),
            })
        }
        Target::Number { label, binding_id } => loop {
            write!(output, "  {label} > ")?;
            output.flush()?;
            if let Ok(value) = read_line(input)?.parse::<f64>() {
                return Ok(Event::ValueChanged {
                    surface_id,
                    binding_id,
                    value: InputValue::Number(value),
                });
            }
        },
    }
}

fn read_line(input: &mut impl BufRead) -> io::Result<String> {
    let mut value = String::new();
    input.read_line(&mut value)?;
    Ok(value.trim().to_string())
}

fn first_empty_input(nodes: &[PresentationNode]) -> Option<Target> {
    for node in nodes {
        match node {
            PresentationNode::Input {
                binding_id,
                label,
                value,
                enabled: true,
                ..
            } if value.is_empty() => {
                return Some(Target::Text {
                    label: label.clone(),
                    binding_id: binding_id.clone(),
                });
            }
            PresentationNode::Qr {
                id,
                purpose: PresentationQrPurpose::Capture,
                label,
                ..
            } => {
                return Some(Target::Text {
                    label: label.clone().unwrap_or_else(|| "QR data".into()),
                    binding_id: id.clone(),
                });
            }
            PresentationNode::Group { children, .. } => {
                if let Some(target) = first_empty_input(children) {
                    return Some(target);
                }
            }
            PresentationNode::List { rows, .. } => {
                for row in rows {
                    if let Some(target) = first_empty_input(&row.controls) {
                        return Some(target);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn collect_targets(node: &PresentationNode, surface_id: &SurfaceId, targets: &mut Vec<Target>) {
    match node {
        PresentationNode::Input {
            binding_id,
            label,
            enabled: true,
            ..
        } => targets.push(Target::Text {
            label: format!("Edit {label}"),
            binding_id: binding_id.clone(),
        }),
        PresentationNode::Toggle {
            binding_id,
            label,
            value,
            enabled: true,
            ..
        } => targets.push(Target::Ready {
            label: label.clone(),
            event: Event::ValueChanged {
                surface_id: surface_id.clone(),
                binding_id: binding_id.clone(),
                value: InputValue::Boolean(!value),
            },
        }),
        PresentationNode::Choice {
            binding_id,
            label,
            options,
            enabled: true,
            ..
        } => targets.extend(options.iter().map(|option| Target::Ready {
            label: format!("{label}: {}", option.label),
            event: Event::ValueChanged {
                surface_id: surface_id.clone(),
                binding_id: binding_id.clone(),
                value: InputValue::Choice(Some(option.id.clone())),
            },
        })),
        PresentationNode::Slider {
            binding_id, label, ..
        } => targets.push(Target::Number {
            label: label.clone(),
            binding_id: binding_id.clone(),
        }),
        PresentationNode::Group { children, .. } => {
            for child in children {
                collect_targets(child, surface_id, targets);
            }
        }
        PresentationNode::List { rows, .. } => {
            for row in rows {
                for control in &row.controls {
                    collect_targets(control, surface_id, targets);
                }
            }
        }
        _ => {}
    }
}

fn default_primary(state: &PresentationState) -> Option<Event> {
    let surface_id = state.surface()?.surface_id.clone();
    let action = state.actions().into_iter().find(|action| {
        action.enabled && action.shortcut == Some(StandardShortcut::ActivatePrimary)
    })?;
    Some(Event::ActionActivated {
        surface_id,
        interaction_id: action.interaction_id,
    })
}
