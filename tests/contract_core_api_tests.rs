// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Consumer contract tests: CLI's expectations of vauchi-core API (CC-05, PI-04).
//!
//! These tests verify that vauchi-core's public API has the shape and behavior
//! that vauchi-cli depends on. If core changes a type, renames a method, or
//! alters return semantics, these tests fail in the CLI repo — catching the
//! break before it reaches production.
//!
//! Contract scope:
//!   - create_identity() initializes identity and own card
//!   - public_id() returns a non-empty String after identity creation
//!   - list_contacts() returns Vec<Contact> with id() and display_name()
//!   - Contact has card() returning ContactCard with display_name() and fields()
//!   - ContactCard serializes/deserializes via serde_json
//!   - FieldType variants that CLI's parse_field_type depends on exist

use vauchi_core::contact_card::ContactAction;
use vauchi_core::network::MockTransport;
use vauchi_core::{Contact, ContactCard, ContactField, FieldType, Vauchi};

/// Helper: create a Vauchi instance with identity.
fn setup() -> Vauchi<MockTransport> {
    let mut wb = Vauchi::in_memory().unwrap();
    wb.create_identity("ContractTest").unwrap();
    wb
}

// ============================================================
// Contract: Identity creation
// ============================================================

#[test]
fn contract_create_identity_succeeds_with_valid_name() {
    let wb = setup();
    // After create_identity, public_id is available
    let public_id = wb.public_id().unwrap();
    assert!(
        !public_id.is_empty(),
        "public_id must be a non-empty String"
    );
}

#[test]
fn contract_identity_has_display_name() {
    let wb = setup();
    let identity = wb
        .identity()
        .expect("identity() must return Some after create");
    assert_eq!(
        identity.display_name(),
        "ContractTest",
        "identity display_name must match what was passed to create_identity"
    );
}

#[test]
fn contract_own_card_exists_after_identity_creation() {
    let wb = setup();
    let card = wb
        .own_card()
        .expect("own_card must not error")
        .expect("own card must exist after identity creation");
    assert_eq!(
        card.display_name(),
        "ContractTest",
        "own card display_name must match identity"
    );
}

// ============================================================
// Contract: Contact listing
// ============================================================

#[test]
fn contract_list_contacts_returns_vec() {
    let wb = setup();
    let contacts: Vec<Contact> = wb
        .list_contacts()
        .expect("list_contacts must return VauchiResult<Vec<Contact>>");
    // Fresh instance has no contacts
    assert!(contacts.is_empty());
}

#[test]
fn contract_list_contacts_paginated_accepts_offset_limit() {
    let wb = setup();
    let contacts: Vec<Contact> = wb
        .list_contacts_paginated(0, 10)
        .expect("list_contacts_paginated(offset, limit) must exist");
    assert!(contacts.is_empty());
}

// ============================================================
// Contract: Contact shape
// ============================================================

#[test]
fn contract_contact_has_required_accessors() {
    // Verify that Contact exposes the methods CLI depends on.
    // We can't easily create a Contact without an exchange, so we verify
    // the type has the expected methods via a compilation check.
    // If any of these methods are renamed/removed, this test fails to compile.
    fn _assert_contact_api(c: &Contact) {
        let _id: &str = c.id();
        let _name: &str = c.display_name();
        let _card: &ContactCard = c.card();
        let _pk: &[u8; 32] = c.public_key();
        let _ts: u64 = c.exchange_timestamp();
        let _hidden: bool = c.is_hidden();
        let _blocked: bool = c.is_blocked();
    }
}

// ============================================================
// Contract: ContactCard shape and serialization
// ============================================================

#[test]
fn contract_contact_card_has_required_accessors() {
    // allow(zero_assertions): Compile-time shape check — fails to compile if API changes
    let card = ContactCard::new("Test");
    let _id: &str = card.id();
    let _name: &str = card.display_name();
    let _fields: &[ContactField] = card.fields();
}

#[test]
fn contract_contact_card_serde_roundtrip() {
    let card = ContactCard::new("Roundtrip");
    let json = serde_json::to_string(&card).expect("ContactCard must serialize to JSON");
    let restored: ContactCard =
        serde_json::from_str(&json).expect("ContactCard must deserialize from JSON");
    assert_eq!(card.id(), restored.id());
    assert_eq!(card.display_name(), restored.display_name());
}

// ============================================================
// Contract: FieldType variants that CLI depends on
// ============================================================

#[test]
fn contract_field_type_variants_exist() {
    // allow(zero_assertions): Compile-time shape check — fails to compile if variants removed
    let _variants = [
        FieldType::Phone,
        FieldType::Email,
        FieldType::Address,
        FieldType::Website,
        FieldType::Social,
        FieldType::Custom,
    ];
}

#[test]
fn contract_contact_field_new_returns_field() {
    let field = ContactField::new(FieldType::Email, "Work", "test@example.com");
    assert_eq!(field.field_type(), FieldType::Email);
    assert_eq!(field.label(), "Work");
    assert_eq!(field.value(), "test@example.com");
}

// ============================================================
// Contract: Card field management
// ============================================================

#[test]
fn contract_add_field_to_own_card() {
    let wb = setup();
    let field = ContactField::new(FieldType::Phone, "Mobile", "+1234567890");
    wb.add_own_field(field)
        .expect("add_field must accept ContactField");

    let card = wb.own_card().unwrap().unwrap();
    assert_eq!(card.fields().len(), 1);
    assert_eq!(card.fields()[0].value(), "+1234567890");
}

// ============================================================
// Contract: Secondary actions API (SP-12a)
// ============================================================

#[test]
fn contract_phone_field_secondary_actions_include_sms() {
    let field = ContactField::new(FieldType::Phone, "Mobile", "+1234567890");
    let actions = field.to_secondary_actions();

    // Phone fields must offer Call, SendSms, and CopyToClipboard
    assert!(
        actions.len() >= 3,
        "phone field must have at least 3 secondary actions, got {}",
        actions.len()
    );
    assert!(
        actions.iter().any(|a| matches!(a, ContactAction::Call(_))),
        "phone secondary actions must include Call"
    );
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, ContactAction::SendSms(_))),
        "phone secondary actions must include SendSms"
    );
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, ContactAction::CopyToClipboard)),
        "phone secondary actions must include CopyToClipboard"
    );
}

#[test]
fn contract_address_field_secondary_actions_include_directions() {
    let field = ContactField::new(FieldType::Address, "Home", "123 Main St, Zurich");
    let actions = field.to_secondary_actions();

    assert!(
        actions
            .iter()
            .any(|a| matches!(a, ContactAction::OpenMap(_))),
        "address secondary actions must include OpenMap"
    );
    assert!(
        actions
            .iter()
            .any(|a| matches!(a, ContactAction::GetDirections(_))),
        "address secondary actions must include GetDirections"
    );
}

#[test]
fn contract_directions_uri_exists_for_address() {
    let field = ContactField::new(FieldType::Address, "Office", "Bahnhofstrasse 1, Zurich");
    let uri = field.to_directions_uri();
    assert!(
        uri.is_some(),
        "to_directions_uri() must return Some for address fields"
    );
    let uri_str = uri.unwrap();
    assert!(
        uri_str.contains("directions"),
        "directions URI must contain 'directions'"
    );
    assert!(
        uri_str.contains("Bahnhofstrasse"),
        "directions URI must contain the address"
    );
}

#[test]
fn contract_get_directions_variant_exists() {
    // allow(zero_assertions): Compile-time shape check — fails to compile if variant removed
    let _action = ContactAction::GetDirections("test".to_string());
}

// ============================================================
// Contract: Consent API that CLI exposes
// ============================================================

#[test]
fn contract_consent_api_shape() {
    use vauchi_core::api::ConsentType;

    let wb = setup();
    wb.grant_consent(ConsentType::Analytics)
        .expect("grant_consent must accept ConsentType");
    let granted: bool = wb
        .check_consent(&ConsentType::Analytics)
        .expect("check_consent must return bool");
    assert!(granted);
}
