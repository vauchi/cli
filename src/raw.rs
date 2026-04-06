// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Machine-readable JSON output for CLI commands.
//!
//! Provides serializable views of core types that exclude internal/crypto
//! fields. Used when `--raw` flag is passed.

use serde::Serialize;
use vauchi_core::{Contact, ContactCard};

/// Serializable view of a [`Contact`] — excludes crypto fields.
#[derive(Serialize)]
pub(crate) struct ContactJson {
    pub id: String,
    pub display_name: String,
    pub fingerprint_verified: bool,
    pub recovery_trusted: bool,
    pub card: CardJson,
}

/// Serializable view of a [`ContactCard`].
#[derive(Serialize)]
pub(crate) struct CardJson {
    pub display_name: String,
    pub fields: Vec<FieldJson>,
}

/// Serializable view of a contact field.
#[derive(Serialize)]
pub(crate) struct FieldJson {
    pub field_type: String,
    pub label: String,
    pub value: String,
}

impl From<&Contact> for ContactJson {
    fn from(c: &Contact) -> Self {
        Self {
            id: c.id().to_string(),
            display_name: c.display_name().to_string(),
            fingerprint_verified: c.is_fingerprint_verified(),
            recovery_trusted: c.is_recovery_trusted(),
            card: CardJson::from(c.card()),
        }
    }
}

impl From<&ContactCard> for CardJson {
    fn from(card: &ContactCard) -> Self {
        Self {
            display_name: card.display_name().to_string(),
            fields: card
                .fields()
                .iter()
                .map(|f| FieldJson {
                    field_type: format!("{:?}", f.field_type()),
                    label: f.label().to_string(),
                    value: f.value().to_string(),
                })
                .collect(),
        }
    }
}

/// Print any serializable value as pretty JSON to stdout.
pub(crate) fn print_json(value: &impl Serialize) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
