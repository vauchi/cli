// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Generic terminal projection of the Core command/event protocol.

// INLINE_TEST_REQUIRED: white-box reducer tests need test-only protocol accessors
// and the injected command reducer without widening the production API.
#[cfg(test)]
use vauchi_core::Event;
use vauchi_core::{
    ActionSpec, Command, ContextBar, OverlaySpec, PresentationNode, SurfaceId, SurfaceSpec,
};
mod interaction;
mod render;
mod session;
pub use interaction::prompt_event;
pub use render::render_to_string;
pub use session::run_engine;
#[cfg(test)]
pub use session::{CommandReducer, run_with_io};
#[derive(Default)]
pub struct PresentationState {
    surface: Option<SurfaceSpec>,
    context_bar: Option<ContextBar>,
    overlay: Option<OverlaySpec>,
    native_back_requested: bool,
}

impl PresentationState {
    pub fn apply(&mut self, commands: &[Command]) -> Vec<Command> {
        let mut effects = Vec::new();
        for command in commands {
            match command {
                Command::ReplaceSurface { surface } => self.replace_surface(surface.clone()),
                Command::SetContextBar {
                    surface_id,
                    revision,
                    bar,
                } if self.is_current_revision(surface_id, *revision) => {
                    self.context_bar = Some((**bar).clone());
                }
                Command::PresentOverlay {
                    surface_id,
                    revision,
                    overlay,
                } if self.is_current_revision(surface_id, *revision) => {
                    self.overlay = Some(overlay.clone());
                }
                Command::SetContextBar { .. } | Command::PresentOverlay { .. } => {}
                Command::PerformNativeBack => self.native_back_requested = true,
                Command::SetPresentationProfile { .. } => {}
                other => effects.push(other.clone()),
            }
        }
        effects
    }

    pub fn surface(&self) -> Option<&SurfaceSpec> {
        self.surface.as_ref()
    }

    #[cfg(test)]
    pub fn context_bar(&self) -> Option<&ContextBar> {
        self.context_bar.as_ref()
    }

    pub fn native_back_requested(&self) -> bool {
        self.native_back_requested
    }

    #[cfg(test)]
    pub fn activation(&self, index: usize) -> Option<Event> {
        let surface_id = self.surface.as_ref()?.surface_id.clone();
        let action = self.actions().get(index)?.clone();
        action.enabled.then_some(Event::ActionActivated {
            surface_id,
            interaction_id: action.interaction_id,
        })
    }

    fn replace_surface(&mut self, candidate: SurfaceSpec) {
        let is_stale = self.surface.as_ref().is_some_and(|current| {
            current.surface_id == candidate.surface_id && current.revision > candidate.revision
        });
        if !is_stale {
            self.surface = Some(candidate);
            self.context_bar = None;
            self.overlay = None;
        }
    }

    fn is_current_revision(&self, surface_id: &SurfaceId, revision: u64) -> bool {
        self.surface.as_ref().is_some_and(|surface| {
            &surface.surface_id == surface_id && surface.revision == revision
        })
    }

    pub(super) fn actions(&self) -> Vec<ActionSpec> {
        if let Some(overlay) = &self.overlay {
            return overlay.items.clone();
        }
        let mut actions = Vec::new();
        if let Some(surface) = &self.surface {
            for node in &surface.nodes {
                collect_node_actions(node, &mut actions);
            }
        }
        if let Some(bar) = &self.context_bar {
            actions.extend(
                [
                    bar.back.as_ref(),
                    bar.navigation.as_ref(),
                    bar.primary.as_ref(),
                    bar.secondary.as_ref(),
                ]
                .into_iter()
                .flatten()
                .cloned(),
            );
        }
        actions
    }
}

fn collect_node_actions(node: &PresentationNode, actions: &mut Vec<ActionSpec>) {
    match node {
        PresentationNode::Group { children, .. } => {
            for child in children {
                collect_node_actions(child, actions);
            }
        }
        PresentationNode::List { rows, .. } => {
            for row in rows {
                if let Some(action) = &row.activation {
                    actions.push(action.clone());
                }
                actions.extend(row.secondary_actions.iter().cloned());
                for control in &row.controls {
                    collect_node_actions(control, actions);
                }
            }
        }
        PresentationNode::Image { activation, .. }
        | PresentationNode::Status { activation, .. } => {
            actions.extend(activation.iter().cloned());
        }
        PresentationNode::Confirmation {
            confirm, cancel, ..
        } => {
            actions.push(confirm.clone());
            actions.push(cancel.clone());
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "presentation_tests.rs"]
mod tests;
