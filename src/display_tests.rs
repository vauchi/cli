// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The text of an activity row and of the aha moment box
//! (vauchi/private#505).

use vauchi_app::notification_types::{ActivityLogEntry, EventOrigin};
use vauchi_core::AhaMomentType;
use vauchi_core::aha_moments::AhaMoment;
use vauchi_core::storage::ActivityLogRow;

use super::{activity_row_line, aha_moment_lines};

fn plain(text: &str) -> String {
    console::strip_ansi_codes(text).to_string()
}

fn row(category: &str, entry: &ActivityLogEntry, contact_id: Option<&str>) -> ActivityLogRow {
    ActivityLogRow {
        event_key: "key".into(),
        category: category.into(),
        contact_id: contact_id.map(Into::into),
        payload: serde_json::to_string(entry).unwrap(),
        created_at: 0,
    }
}

// @internal
#[test]
fn each_activity_category_gets_its_icon_and_title() {
    let cases = [
        (
            "emergency_alert_received",
            ActivityLogEntry::EmergencyAlertReceived {
                contact_id: "c".into(),
            },
            "🚨",
            "EMERGENCY ALERT",
        ),
        (
            "contact_added",
            ActivityLogEntry::ContactAdded {
                contact_id: "c".into(),
                origin: EventOrigin::Local,
            },
            "👤",
            "New Contact Added",
        ),
        (
            "card_update_received",
            ActivityLogEntry::CardUpdateReceived {
                contact_id: "c".into(),
                changed_fields: vec!["email".into(), "phone".into()],
            },
            "📥",
            "Card Update: email, phone",
        ),
        (
            "card_update_failed",
            ActivityLogEntry::CardUpdateFailed {
                contact_id: "c".into(),
                reason: "offline".into(),
            },
            "⚠",
            "Card Update Failed: offline",
        ),
        (
            "own_card_updated",
            ActivityLogEntry::OwnCardUpdated {
                changed_fields: vec!["email".into()],
            },
            "✏",
            "You Updated Your Card: email",
        ),
        (
            "contact_removed",
            ActivityLogEntry::ContactRemoved {
                contact_id: "c".into(),
            },
            "🗑",
            "Contact Removed",
        ),
    ];

    for (category, entry, icon, title) in cases {
        let line = plain(&activity_row_line(&row(category, &entry, None)));
        assert_eq!(
            line,
            format!("{icon} [1970-01-01 00:00:00] {title}"),
            "{category}"
        );
    }
}

// @internal
#[test]
fn updates_without_changed_fields_use_the_plain_title() {
    let received = ActivityLogEntry::CardUpdateReceived {
        contact_id: "c".into(),
        changed_fields: Vec::new(),
    };
    let own = ActivityLogEntry::OwnCardUpdated {
        changed_fields: Vec::new(),
    };

    assert!(
        plain(&activity_row_line(&row(
            "card_update_received",
            &received,
            None
        )))
        .ends_with("Card Update Received")
    );
    assert!(
        plain(&activity_row_line(&row("own_card_updated", &own, None)))
            .ends_with("You Updated Your Card")
    );
}

// @internal
#[test]
fn an_unknown_category_shows_its_name_and_the_contact_prefix() {
    let entry = ActivityLogEntry::CardUpdatePending {
        contact_id: "c".into(),
    };

    let line = plain(&activity_row_line(&row(
        "card_update_pending",
        &entry,
        Some("0123456789abcdef"),
    )));

    assert_eq!(
        line,
        "• [1970-01-01 00:00:00] card_update_pending for contact 01234567"
    );
}

// @internal
#[test]
fn the_aha_box_is_52_columns_on_every_line() {
    let moment = AhaMoment::with_context(AhaMomentType::FirstContactAdded, "Bob".into());

    let lines: Vec<String> = aha_moment_lines(&moment).iter().map(|l| plain(l)).collect();

    assert!(
        lines[1].starts_with(&format!("│ ★ {}", moment.title())),
        "{lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains("You now have Bob's card"))
    );
    for line in &lines {
        assert_eq!(line.chars().count(), 52, "{line:?}");
    }
}
