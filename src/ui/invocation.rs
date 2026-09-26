// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

//! One-shot presentation of Core-applied invocations.

use std::io::Write;

use vauchi_core::{Command, InvocationOutcome};

pub fn exit_code(_outcome: InvocationOutcome) -> u8 {
    unimplemented!()
}

pub fn present(_commands: &[Command], _out: &mut impl Write, _err: &mut impl Write) -> u8 {
    unimplemented!()
}

// INLINE_TEST_REQUIRED: the CLI is a binary crate with no library target,
// so an integration test cannot reach this module's writer-injected API.
#[cfg(test)]
#[path = "invocation_tests.rs"]
mod tests;
