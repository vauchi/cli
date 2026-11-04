// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
// SPDX-License-Identifier: GPL-3.0-or-later

use vauchi_cli_display::presentation::{PresentationState, render_to_string};
use vauchi_contract::Command;

// @scenario: generic_presentation_protocol.feature :: Every shell renders the same prepared presentation
#[test]
fn cli_display_replays_the_contract_fixture_without_an_engine() {
    let fixture: serde_json::Value =
        serde_json::from_str(vauchi_contract::fixtures::presentation_contract_v1_json()).unwrap();
    let mut state = PresentationState::default();
    let initial: Vec<Command> =
        serde_json::from_value(fixture["initial_commands"].clone()).unwrap();
    assert_eq!(state.apply(&initial), Vec::<Command>::new());
    assert!(render_to_string(&state).contains("Create new identity"));
    for step in fixture["steps"].as_array().unwrap() {
        let commands: Vec<Command> = serde_json::from_value(step["commands"].clone()).unwrap();
        assert_eq!(state.apply(&commands), Vec::<Command>::new());
    }
    assert_eq!(
        serde_json::to_value(state.surface().unwrap()).unwrap(),
        fixture["expected_state"]["surface"]
    );
    assert_eq!(
        serde_json::to_value(state.context_bar().unwrap()).unwrap(),
        fixture["expected_state"]["context_bar"]
    );
}
