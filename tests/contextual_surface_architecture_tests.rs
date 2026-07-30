// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::path::Path;

// @scenario: generic_presentation_protocol.feature :: Release contains only the generic action system
#[test]
fn device_replacement_uses_only_the_generic_core_reducer_boundary() {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands/device_replacement.rs"),
    )
    .expect("device replacement source");

    assert!(source.contains("AppEngine::for_device_replacement"));
    for retired_boundary in [
        "ActionResult",
        "DeviceReplacementEngine",
        "ScreenModel",
        "UserAction",
        "WorkflowEngine",
        "action_handler",
        "screen_renderer",
    ] {
        assert!(
            !source.contains(retired_boundary),
            "device replacement still references retired boundary {retired_boundary}"
        );
    }
}
