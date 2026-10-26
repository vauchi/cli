// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use vauchi_core::PresentationColors;

pub(crate) fn test_colors() -> Box<PresentationColors> {
    Box::new(PresentationColors {
        background: "#282a36".into(),
        surface: "#21222c".into(),
        text: "#f8f8f2".into(),
        text_muted: "#bfbfbf".into(),
        accent: "#bd93f9".into(),
        border: "#6272a4".into(),
        success: "#50fa7b".into(),
        warning: "#ffb86c".into(),
        error: "#ff5555".into(),
        surface_input: None,
        surface_muted: None,
        surface_elevated: None,
        text_on_accent: None,
        focus_ring: None,
    })
}
