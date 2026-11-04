// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{CommandReducer, run_with_io};
use vauchi_core::{
    AccessibilitySpec, ActionSpec, ActionTone, Command, ContextBar, Event, FilePickPurpose,
    InteractionId, PresentationNode, PresentationTextStyle, PresentationTokens, StandardShortcut,
    SurfaceId, SurfaceLayout, SurfaceSpec,
};

fn action(id: &str, label: &str) -> ActionSpec {
    ActionSpec {
        interaction_id: InteractionId::new(id).unwrap(),
        label: label.into(),
        accessibility_label: label.into(),
        icon_token: None,
        enabled: true,
        tone: ActionTone::Standard,
        shortcut: Some(StandardShortcut::ActivatePrimary),
    }
}

fn surface(revision: u64, title: &str) -> SurfaceSpec {
    SurfaceSpec {
        surface_id: SurfaceId::new("welcome").unwrap(),
        revision,
        title: title.into(),
        subtitle: Some("Prepared by Core".into()),
        accessibility_label: title.into(),
        layout: SurfaceLayout::Scroll,
        tokens: PresentationTokens {
            spacing_small: 1,
            spacing_medium: 2,
            spacing_large: 3,
            corner_radius: 0,
            minimum_target_size: 1,
            colors: crate::ui::test_colors::test_colors(),
            fonts: None,
        },
        nodes: vec![PresentationNode::Text {
            id: None,
            content: "Hello".into(),
            style: PresentationTextStyle::Body,
            accessibility: AccessibilitySpec::label("Hello"),
        }],
    }
}

struct FakeReducer {
    initial: Vec<Command>,
}

impl CommandReducer for FakeReducer {
    type Error = std::convert::Infallible;

    fn initial_commands(&mut self) -> Result<Vec<Command>, Self::Error> {
        Ok(std::mem::take(&mut self.initial))
    }

    fn dispatch(&mut self, event: Event) -> Result<Vec<Command>, Self::Error> {
        assert!(matches!(
            event,
            Event::ActionActivated { interaction_id, .. }
                if interaction_id.as_str() == "opaque-token"
        ));
        Ok(vec![Command::PerformNativeBack])
    }
}

// @internal
#[test]
fn generic_session_exits_only_when_core_requests_native_back() {
    let mut reducer = FakeReducer {
        initial: vec![
            Command::ReplaceSurface {
                surface: surface(1, "Session"),
            },
            Command::SetContextBar {
                surface_id: SurfaceId::new("welcome").unwrap(),
                revision: 1,
                bar: Box::new(ContextBar {
                    primary: Some(action("opaque-token", "Continue")),
                    ..ContextBar::default()
                }),
            },
        ],
    };
    let mut input = std::io::Cursor::new(b"1\n");
    let mut output = Vec::new();

    run_with_io(&mut reducer, &mut input, &mut output).expect("generic session");

    assert!(String::from_utf8(output).unwrap().contains("Session"));
}

struct FilePickReducer {
    expected_bytes: Vec<u8>,
    expected_filename: String,
}

impl CommandReducer for FilePickReducer {
    type Error = std::convert::Infallible;

    fn initial_commands(&mut self) -> Result<Vec<Command>, Self::Error> {
        Ok(vec![Command::FilePickFromUser {
            accepted_mime_types: vec!["application/octet-stream".into()],
            accepted_extensions: vec!["vbk".into()],
            max_bytes: None,
            purpose: FilePickPurpose::ImportBackup,
        }])
    }

    fn dispatch(&mut self, event: Event) -> Result<Vec<Command>, Self::Error> {
        assert_eq!(
            event,
            Event::FilePickedFromUser {
                bytes: self.expected_bytes.clone(),
                filename: self.expected_filename.clone(),
            }
        );
        Ok(vec![Command::PerformNativeBack])
    }
}

// @internal
#[test]
fn generic_session_adapts_a_core_file_pick_to_terminal_input() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let path = directory.path().join("backup.vauchi");
    let bytes = b"encrypted backup".to_vec();
    std::fs::write(&path, &bytes).expect("test backup");
    let mut reducer = FilePickReducer {
        expected_bytes: bytes,
        expected_filename: "backup.vauchi".into(),
    };
    let mut input = std::io::Cursor::new(format!("{}\n", path.display()));
    let mut output = Vec::new();

    run_with_io(&mut reducer, &mut input, &mut output).expect("file-pick session");

    assert!(
        String::from_utf8(output)
            .expect("utf-8 terminal output")
            .contains("File path")
    );
}
