// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io;

use vauchi_contract::{ContextBar, Event, InputValue, OverlaySpec, SurfaceSpec};

use super::{
    OfferedInputs, PresentationState, Target, collect_targets, default_primary, first_empty_input,
};

/// A prompt that accepts completed lines without owning or waiting on stdin.
/// The host retains it across idle ticks and replaces it when presentation changes.
pub struct LinePrompt {
    surface: SurfaceSpec,
    context_bar: Option<ContextBar>,
    overlay: Option<OverlaySpec>,
    targets: Vec<Target>,
    primary: Option<Event>,
    stage: Stage,
    prompt: String,
}

enum Stage {
    Selection,
    Entry(Target),
    Finished,
}

impl LinePrompt {
    pub fn new(state: &PresentationState, offered: &mut OfferedInputs) -> io::Result<Self> {
        let surface = state
            .surface()
            .ok_or_else(|| io::Error::other("Core has not presented a surface"))?;
        let mut targets = Vec::new();
        if state.overlay().is_none() {
            for node in &surface.nodes {
                collect_targets(node, &surface.surface_id, &mut targets);
            }
        }
        targets.extend(
            state
                .actions()
                .into_iter()
                .filter(|action| action.enabled)
                .map(|action| Target::Ready {
                    label: action.label,
                    event: Event::ActionActivated {
                        surface_id: surface.surface_id.clone(),
                        interaction_id: action.interaction_id,
                    },
                }),
        );
        offered.enter_surface(&surface.surface_id);
        let entry = state
            .overlay()
            .is_none()
            .then(|| first_empty_input(&surface.nodes, offered))
            .flatten();
        let (stage, prompt) = if let Some(target) = entry {
            offered.mark(&target);
            let prompt = format!("  {} > ", target.label());
            (Stage::Entry(target), prompt)
        } else if targets.is_empty() {
            let prompt = state.overlay().map_or_else(
                || "  Enter to go back > ".into(),
                |overlay| format!("  {} > ", overlay.close_label.as_deref().unwrap_or("×")),
            );
            (Stage::Selection, prompt)
        } else {
            let mut prompt = String::new();
            for (index, target) in targets.iter().enumerate() {
                prompt.push_str(&format!("  {}. {}\n", index + 1, target.label()));
            }
            prompt.push_str(&format!("  Choose (1-{}) > ", targets.len()));
            (Stage::Selection, prompt)
        };
        Ok(Self {
            surface: surface.clone(),
            context_bar: state.context_bar().cloned(),
            overlay: state.overlay().cloned(),
            targets,
            primary: default_primary(state),
            stage,
            prompt,
        })
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    pub fn is_secret(&self) -> bool {
        matches!(&self.stage, Stage::Entry(Target::Text { secret: true, .. }))
    }

    pub fn is_current(&self, state: &PresentationState) -> bool {
        state.surface() == Some(&self.surface)
            && state.context_bar() == self.context_bar.as_ref()
            && state.overlay() == self.overlay.as_ref()
    }

    /// `line` excludes its transport newline. Text, including secrets, is preserved verbatim.
    /// `None` requests another line; an event completes this prompt exactly once.
    pub fn submit_line(
        &mut self,
        state: &PresentationState,
        line: &str,
    ) -> io::Result<Option<Event>> {
        if !self.is_current(state) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "presentation changed while awaiting input",
            ));
        }
        if matches!(self.stage, Stage::Finished) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "prompt already completed",
            ));
        }
        if matches!(self.stage, Stage::Selection) {
            if line.trim().is_empty() {
                if let Some(event) = self.primary.clone() {
                    return Ok(self.complete(event));
                }
                if self.targets.is_empty() {
                    let surface_id = self.surface.surface_id.clone();
                    let event = self.overlay.as_ref().map_or_else(
                        || Event::BackRequested {
                            surface_id: surface_id.clone(),
                        },
                        |overlay| Event::OverlayDismissed {
                            surface_id: surface_id.clone(),
                            kind: overlay.kind,
                        },
                    );
                    return Ok(self.complete(event));
                }
            }
            let target = line
                .trim()
                .parse::<usize>()
                .ok()
                .and_then(|index| index.checked_sub(1))
                .and_then(|index| self.targets.get(index))
                .cloned();
            match target {
                Some(Target::Ready { event, .. }) => return Ok(self.complete(event)),
                Some(target) => {
                    self.prompt = format!("  {} > ", target.label());
                    self.stage = Stage::Entry(target);
                }
                None => self.prompt = "  Invalid selection > ".into(),
            }
            return Ok(None);
        }
        let Stage::Entry(target) = &self.stage else {
            unreachable!()
        };
        let (binding_id, value) = match target {
            Target::Text { binding_id, .. } => (binding_id.clone(), InputValue::Text(line.into())),
            Target::Number { binding_id, .. } => {
                let Some(value) = line
                    .trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                else {
                    self.prompt = format!("  Invalid number. {} > ", target.label());
                    return Ok(None);
                };
                (binding_id.clone(), InputValue::Number(value))
            }
            Target::Ready { .. } => unreachable!(),
        };
        Ok(self.complete(Event::ValueChanged {
            surface_id: self.surface.surface_id.clone(),
            binding_id,
            value,
        }))
    }

    fn complete(&mut self, event: Event) -> Option<Event> {
        self.stage = Stage::Finished;
        self.prompt.clear();
        Some(event)
    }
}
