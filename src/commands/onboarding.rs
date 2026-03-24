// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Onboarding Command
//!
//! Interactive onboarding flow driven by the core OnboardingEngine.
//! Renders screens to stdout and reads user input from stdin.

use anyhow::Result;

use vauchi_app::ui::{ActionResult, OnboardingEngine, WorkflowEngine};

use crate::display;
use crate::ui::action_handler;
use crate::ui::screen_renderer;

/// Runs the interactive onboarding flow.
pub fn run() -> Result<()> {
    let mut engine = OnboardingEngine::new();

    loop {
        let screen = engine.current_screen();

        // Render the current screen
        screen_renderer::render(&screen);

        // Get user input
        let action = action_handler::prompt_for_action(&screen)
            .map_err(|e| anyhow::anyhow!("Input error: {}", e))?;

        // Resolve toggle placeholders
        let action = action_handler::resolve_toggle_item_id(action, &screen);

        // Handle the action
        match engine.handle_action(action) {
            ActionResult::UpdateScreen(_) => {
                // Screen updated in place, loop will re-render
            }
            ActionResult::NavigateTo(_) => {
                // Navigation handled, loop will render new screen
            }
            ActionResult::ValidationError {
                component_id,
                message,
            } => {
                display::warning(&format!("{}: {}", component_id, message));
            }
            ActionResult::Complete => {
                println!();
                let data = engine.data();
                display::success(&format!(
                    "Onboarding complete! Welcome, {}.",
                    data.display_name
                ));

                if !data.selected_groups.is_empty() {
                    let selected: Vec<&str> = data
                        .selected_groups
                        .iter()
                        .filter(|g| g.selected)
                        .map(|g| g.name.as_str())
                        .collect();
                    if !selected.is_empty() {
                        display::info(&format!("Groups: {}", selected.join(", ")));
                    }
                }

                display::info("Run 'vauchi init <name>' to create your identity.");
                println!();
                break;
            }
            ActionResult::ShowAlert { title, message } => {
                display::warning(&format!("{}: {}", title, message));
            }
            ActionResult::OpenUrl { url } => {
                display::info(&format!("Open: {}", url));
            }
            _ => {
                // StartDeviceLink, StartBackupImport, OpenContact,
                // RequestCamera, WipeComplete — not applicable in CLI
            }
        }
    }

    Ok(())
}
