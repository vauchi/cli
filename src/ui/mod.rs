// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Generic terminal adapter for the Core command/event protocol.

pub mod invocation;
pub mod presentation;
// INLINE_TEST_REQUIRED: the extracted white-box test modules share one
// theme-colours fixture for the surfaces they build.
#[cfg(test)]
pub(crate) mod test_colors;
