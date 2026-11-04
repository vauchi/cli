// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

pub use vauchi_cli_display::presentation::*;
mod session;
pub use session::run_engine;
// INLINE_TEST_REQUIRED: the binary's injected reducer tests exercise the app
// session and native effects without exposing its engine adapter.
#[cfg(test)]
pub use session::{CommandReducer, run_with_io};
#[cfg(test)]
#[path = "presentation_session_tests.rs"]
mod tests;
