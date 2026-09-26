// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

//! One-shot presentation of Core-applied invocations.

use std::io::Write;

use vauchi_core::{Command, InvocationOutcome};

use super::presentation::{PresentationState, render_to_string};

/// ADR-066 Amendment 2026-09-26 (b): 2 matches clap's usage-error code;
/// 69 and 77 are sysexits' EX_UNAVAILABLE and EX_NOPERM.
pub fn exit_code(outcome: InvocationOutcome) -> u8 {
    match outcome {
        InvocationOutcome::Succeeded => 0,
        InvocationOutcome::Failed => 1,
        InvocationOutcome::InvalidInput => 2,
        InvocationOutcome::Unavailable => 69,
        InvocationOutcome::Denied => 77,
    }
}

/// Write Core's one-shot commands to the terminal streams and return the
/// exit code. stdout carries only the document or the rendered surface.
pub fn present(commands: &[Command], out: &mut impl Write, err: &mut impl Write) -> u8 {
    let mut carried_everything = true;
    for command in commands {
        let written = match command {
            Command::EmitDocument { document } => {
                let newline: &[u8] = if document.data.ends_with(b"\n") {
                    b""
                } else {
                    b"\n"
                };
                out.write_all(&document.data)
                    .and_then(|()| out.write_all(newline))
            }
            Command::ReplaceSurface { .. } => {
                let mut state = PresentationState::default();
                state.apply(std::slice::from_ref(command));
                out.write_all(render_to_string(&state).as_bytes())
            }
            Command::PresentAlert { alert } => writeln!(err, "{}: {}", alert.title, alert.message),
            Command::ShowToast { toast } => writeln!(err, "{}", toast.message),
            Command::FinishInvocation { outcome } => {
                let code = exit_code(*outcome);
                return if carried_everything || code != 0 {
                    code
                } else {
                    1
                };
            }
            unsupported => {
                carried_everything = false;
                writeln!(
                    err,
                    "vauchi: this terminal cannot carry {}",
                    unsupported.variant_name()
                )
            }
        };
        if written.is_err() {
            return 1;
        }
    }
    let _ = writeln!(err, "vauchi: invocation ended without an outcome");
    1
}

// INLINE_TEST_REQUIRED: the CLI is a binary crate with no library target,
// so an integration test cannot reach this module's writer-injected API.
#[cfg(test)]
#[path = "invocation_tests.rs"]
mod tests;
