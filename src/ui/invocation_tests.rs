// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use vauchi_core::{
    AlertSpec, Command, DocumentSpec, InvocationOutcome, PresentationTokens, SurfaceId,
    SurfaceLayout, SurfaceSpec,
};

use super::{exit_code, present};

fn presented(commands: &[Command]) -> (u8, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = present(commands, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).expect("utf8 stdout"),
        String::from_utf8(err).expect("utf8 stderr"),
    )
}

fn finish(outcome: InvocationOutcome) -> Command {
    Command::FinishInvocation { outcome }
}

fn surface(title: &str) -> Command {
    Command::ReplaceSurface {
        surface: SurfaceSpec {
            surface_id: SurfaceId::new("one-shot").expect("id"),
            revision: 1,
            title: title.into(),
            subtitle: None,
            accessibility_label: title.into(),
            layout: SurfaceLayout::Scroll,
            tokens: PresentationTokens {
                spacing_small: 4,
                spacing_medium: 8,
                spacing_large: 16,
                corner_radius: 8,
                minimum_target_size: 44,
            },
            nodes: Vec::new(),
        },
    }
}

#[test]
fn outcomes_map_to_the_adr_066_exit_codes() {
    assert_eq!(exit_code(InvocationOutcome::Succeeded), 0);
    assert_eq!(exit_code(InvocationOutcome::Failed), 1);
    assert_eq!(exit_code(InvocationOutcome::InvalidInput), 2);
    assert_eq!(exit_code(InvocationOutcome::Unavailable), 69);
    assert_eq!(exit_code(InvocationOutcome::Denied), 77);
}

#[test]
fn a_document_is_the_only_thing_on_stdout() {
    let (code, out, err) = presented(&[
        Command::EmitDocument {
            document: DocumentSpec {
                media_type: "application/json".into(),
                schema: "vauchi.contacts.v1".into(),
                data: b"[]".to_vec(),
            },
        },
        finish(InvocationOutcome::Succeeded),
    ]);

    assert_eq!((code, out.as_str(), err.as_str()), (0, "[]\n", ""));
}

#[test]
fn a_surface_is_rendered_once_to_stdout() {
    let (code, out, err) = presented(&[
        surface("Contacts (0):"),
        finish(InvocationOutcome::Succeeded),
    ]);

    assert_eq!(code, 0);
    assert!(out.contains("Contacts (0):"), "got: {out}");
    assert_eq!(err, "");
}

#[test]
fn alerts_go_to_stderr_and_the_outcome_sets_the_code() {
    let (code, out, err) = presented(&[
        Command::PresentAlert {
            alert: AlertSpec {
                title: "Something went wrong".into(),
                message: "Unknown error".into(),
            },
        },
        finish(InvocationOutcome::Failed),
    ]);

    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert_eq!(err, "Something went wrong: Unknown error\n");
}

#[test]
fn a_run_without_an_outcome_fails_closed() {
    let (code, _, err) = presented(&[surface("Partial")]);

    assert_eq!(code, 1);
    assert!(err.contains("without an outcome"), "got: {err}");
}

#[test]
fn a_command_the_terminal_cannot_carry_fails_the_run() {
    let (code, _, err) = presented(&[Command::QrRequestScan, finish(InvocationOutcome::Succeeded)]);

    assert_eq!(
        code, 1,
        "silently dropping part of Core's output is not success"
    );
    assert!(err.contains("QrRequestScan"), "got: {err}");
}
