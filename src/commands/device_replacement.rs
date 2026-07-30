// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Device Replacement Wizard
//!
//! Interactive device replacement flows driven by the Core reducer. Three modes:
//! - **setup**: Run on the OLD device to generate a transfer QR.
//! - **transfer**: Run on the NEW device to receive data.
//! - **post-restore**: After restoring from backup, show recovery guidance.

use anyhow::Result;

use vauchi_app::ui::{AppEngine, ReplacementRole};
use vauchi_core::api::Vauchi;

use crate::ui::presentation;

/// Runs the source (old device) replacement wizard.
pub fn run_setup() -> Result<()> {
    run_wizard(ReplacementRole::Source)
}

/// Runs the target (new device) replacement wizard.
pub fn run_transfer() -> Result<()> {
    run_wizard(ReplacementRole::Target)
}

/// Runs the post-restore guidance wizard.
pub fn run_post_restore() -> Result<()> {
    run_wizard(ReplacementRole::PostRestore)
}

fn run_wizard(role: ReplacementRole) -> Result<()> {
    let mut engine = AppEngine::for_device_replacement(Vauchi::in_memory()?, role);
    presentation::run_engine(&mut engine)
}
