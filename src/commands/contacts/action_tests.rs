// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What each contact action is called, which URI it opens, what the CLI
//! reports and what it shows for manual copying (vauchi/private#505).

use vauchi_core::contact_card::ContactAction;

use super::{action_label, action_uri, action_value, opened_description, url_encode_value};

fn every_action() -> Vec<ContactAction> {
    vec![
        ContactAction::Call("+41 79 123".into()),
        ContactAction::SendSms("+41 79 123".into()),
        ContactAction::SendEmail("a@b.ch".into()),
        ContactAction::OpenUrl("https://vauchi.app".into()),
        ContactAction::OpenMap("Main St 1".into()),
        ContactAction::GetDirections("Main St 1".into()),
        ContactAction::CopyToClipboard,
    ]
}

// @internal
#[test]
fn each_action_has_its_label() {
    let labels: Vec<String> = every_action().iter().map(action_label).collect();

    assert_eq!(
        labels,
        [
            "Call +41 79 123",
            "Send SMS to +41 79 123",
            "Email a@b.ch",
            "Open https://vauchi.app",
            "Open in Maps: Main St 1",
            "Get Directions to Main St 1",
            "Copy to Clipboard",
        ]
    );
}

// @internal
#[test]
fn each_action_opens_its_uri_and_copying_opens_nothing() {
    let uris: Vec<Option<String>> = every_action().iter().map(action_uri).collect();

    assert_eq!(
        uris,
        [
            Some("tel:+41 79 123".to_string()),
            Some("sms:+41 79 123".to_string()),
            Some("mailto:a@b.ch".to_string()),
            Some("https://vauchi.app".to_string()),
            Some("https://www.openstreetmap.org/search?query=Main%20St%201".to_string()),
            Some("https://www.openstreetmap.org/directions?route=&to=Main%20St%201".to_string()),
            None,
        ]
    );
}

// @internal
#[test]
fn each_opened_action_is_reported_by_kind() {
    let reports: Vec<&str> = every_action().iter().map(opened_description).collect();

    assert_eq!(
        reports,
        [
            "Opened dialer",
            "Opened messaging",
            "Opened email client",
            "Opened browser",
            "Opened maps",
            "Opened directions",
            "Opened",
        ]
    );
}

// @internal
#[test]
fn each_action_shows_its_raw_value_for_copying() {
    let actions = every_action();
    let values: Vec<&str> = actions.iter().map(action_value).collect();

    assert_eq!(
        values,
        [
            "+41 79 123",
            "+41 79 123",
            "a@b.ch",
            "https://vauchi.app",
            "Main St 1",
            "Main St 1",
            "",
        ]
    );
}

// @internal
#[test]
fn values_are_percent_encoded_except_unreserved_characters() {
    assert_eq!(url_encode_value("a b&c?d#e"), "a%20b%26c%3Fd%23e");
    assert_eq!(url_encode_value("Az09-._~,+/"), "Az09-._~,+/");
    assert_eq!(url_encode_value("ü@"), "%FC%40");
}
