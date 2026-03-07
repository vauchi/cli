// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! CLI text renderer for core-driven UI.
//!
//! Maps core UI types (ScreenModel, Component) to formatted terminal output
//! and reads user input back as UserAction.

pub mod action_handler;
pub mod screen_renderer;
