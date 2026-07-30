// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Onboarding Command
//!
//! Interactive onboarding flow driven by the canonical Core reducer.

use anyhow::Result;

use vauchi_app::ui::AppEngine;
use vauchi_core::api::Vauchi;

use crate::ui::presentation;

/// Runs the interactive onboarding flow.
pub fn run() -> Result<()> {
    let mut engine = AppEngine::new(Vauchi::in_memory()?);
    presentation::run_engine(&mut engine)
}
