// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Device Replacement Wizard
//!
//! Interactive device replacement flows driven by the core
//! `DeviceReplacementEngine`. Three modes:
//! - **setup**: Run on the OLD device to generate a transfer QR.
//! - **transfer**: Run on the NEW device to receive data.
//! - **post-restore**: After restoring from backup, show recovery guidance.

use anyhow::Result;

use vauchi_app::ui::{ActionResult, DeviceReplacementEngine, WorkflowEngine};

use crate::display;
use crate::ui::action_handler;
use crate::ui::screen_renderer;

/// Runs the source (old device) replacement wizard.
pub fn run_setup() -> Result<()> {
    run_wizard(DeviceReplacementEngine::new_source())
}

/// Runs the target (new device) replacement wizard.
pub fn run_transfer() -> Result<()> {
    run_wizard(DeviceReplacementEngine::new_target())
}

/// Runs the post-restore guidance wizard.
pub fn run_post_restore() -> Result<()> {
    run_wizard(DeviceReplacementEngine::new_post_restore())
}

fn run_wizard(mut engine: DeviceReplacementEngine) -> Result<()> {
    loop {
        let screen = engine.current_screen();
        screen_renderer::render(&screen);

        let action = action_handler::prompt_for_action(&screen)
            .map_err(|e| anyhow::anyhow!("Input error: {}", e))?;

        let action = action_handler::resolve_toggle_item_id(action, &screen);

        match engine.handle_action(action) {
            ActionResult::UpdateScreen(_) | ActionResult::NavigateTo(_) => {}
            ActionResult::ValidationError {
                component_id,
                message,
            } => {
                display::warning(&format!("{}: {}", component_id, message));
            }
            ActionResult::Complete => {
                println!();
                if engine.was_cancelled() {
                    display::info("Device replacement cancelled.");
                } else {
                    match engine.completion_outcome() {
                        vauchi_app::ui::CompletionOutcome::RemoveOldDevice => {
                            display::success(
                                "Transfer complete. This device has been marked for removal.",
                            );
                            display::info("Run 'vauchi device revoke' to finalize the removal.");
                        }
                        vauchi_app::ui::CompletionOutcome::KeepBoth => {
                            display::success("Transfer complete. Both devices remain linked.");
                        }
                        vauchi_app::ui::CompletionOutcome::Cancelled => {
                            display::info("Device replacement cancelled.");
                        }
                    }
                }
                println!();
                break;
            }
            ActionResult::StartDeviceLink => {
                println!();
                display::info("To complete the transfer, use the device link flow:");
                display::info("  On this device:  vauchi device join <qr_data>");
                display::info("  On old device:   vauchi device complete <request>");
                println!();
                break;
            }
            ActionResult::ExchangeCommands { commands } => {
                use vauchi_core::exchange::{ExchangeCommand, FilePickPurpose};
                let backup_pick = commands.iter().any(|c| {
                    matches!(
                        c,
                        ExchangeCommand::FilePickFromUser {
                            purpose: FilePickPurpose::ImportBackup,
                            ..
                        }
                    )
                });
                if backup_pick {
                    println!();
                    display::info("To restore from backup:");
                    display::info("  vauchi import <backup_file>");
                    display::info(
                        "Then run 'vauchi device replace post-restore' for recovery guidance.",
                    );
                    println!();
                    break;
                }
            }
            ActionResult::ShowAlert { title, message } => {
                display::warning(&format!("{}: {}", title, message));
            }
            _ => {}
        }
    }

    Ok(())
}
