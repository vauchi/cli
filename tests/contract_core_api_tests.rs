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
    // allow(zero_assertions): Compile-time shape check — fails to compile if API changes
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
    wb.grant_consent(ConsentType::RecoveryVouching)
        .expect("grant_consent must accept ConsentType");
    let granted: bool = wb
        .check_consent(&ConsentType::RecoveryVouching)
        .expect("check_consent must return bool");
    assert!(granted);
}

// ============================================================
// Contract: Contact Merge API (SP-12a)
// ============================================================

#[test]
fn contract_find_duplicates_returns_pairs() {
    use vauchi_core::contact::merge::{find_duplicates, DuplicatePair};
    use vauchi_core::crypto::SymmetricKey;

    // Create contacts with similar names
    let card1 = vauchi_core::ContactCard::new("Alice Johnson");
    let card2 = vauchi_core::ContactCard::new("Alice Johnson"); // exact match
    let card3 = vauchi_core::ContactCard::new("Bob Smith");

    let c1 = Contact::from_exchange([1u8; 32], card1, SymmetricKey::generate());
    let c2 = Contact::from_exchange([2u8; 32], card2, SymmetricKey::generate());
    let c3 = Contact::from_exchange([3u8; 32], card3, SymmetricKey::generate());

    let duplicates: Vec<DuplicatePair> = find_duplicates(&[c1, c2, c3]);

    // Exact name match should produce a duplicate pair
    assert!(
        !duplicates.is_empty(),
        "find_duplicates must detect exact name matches"
    );
    assert!(
        duplicates[0].similarity >= 0.7,
        "similarity must be >= 0.7 threshold, got {}",
        duplicates[0].similarity
    );
}

#[test]
fn contract_merge_contacts_preserves_primary_name() {
    use vauchi_core::contact::merge::merge_contacts;
    use vauchi_core::crypto::SymmetricKey;

    let card1 = vauchi_core::ContactCard::new("Primary Contact");
    let card2 = vauchi_core::ContactCard::new("Secondary Contact");

    let primary = Contact::from_exchange([1u8; 32], card1, SymmetricKey::generate());
    let secondary = Contact::from_exchange([2u8; 32], card2, SymmetricKey::generate());

    let merged = merge_contacts(&primary, &secondary);

    assert_eq!(
        merged.display_name(),
        "Primary Contact",
        "merged contact must keep primary's display name"
    );
    assert_eq!(
        merged.id(),
        primary.id(),
        "merged contact must keep primary's ID"
    );
}

#[test]
fn contract_merge_contacts_adds_unique_fields() {
    use vauchi_core::contact::merge::merge_contacts;
    use vauchi_core::crypto::SymmetricKey;

    let mut card1 = vauchi_core::ContactCard::new("Primary");
    card1
        .add_field(ContactField::new(FieldType::Email, "Work", "p@example.com"))
        .unwrap();

    let mut card2 = vauchi_core::ContactCard::new("Secondary");
    card2
        .add_field(ContactField::new(FieldType::Phone, "Mobile", "+1234567890"))
        .unwrap();

    let primary = Contact::from_exchange([1u8; 32], card1, SymmetricKey::generate());
    let secondary = Contact::from_exchange([2u8; 32], card2, SymmetricKey::generate());

    let merged = merge_contacts(&primary, &secondary);

    assert_eq!(
        merged.card().fields().len(),
        2,
        "merged card must contain fields from both contacts"
    );
    assert!(
        merged.card().fields().iter().any(|f| f.label() == "Work"),
        "merged card must keep primary's email field"
    );
    assert!(
        merged.card().fields().iter().any(|f| f.label() == "Mobile"),
        "merged card must add secondary's phone field"
    );
}

#[test]
fn contract_filter_dismissed_excludes_dismissed_pairs() {
    use vauchi_core::contact::merge::{filter_dismissed, normalize_pair_key, DuplicatePair};

    let pairs = vec![
        DuplicatePair {
            id1: "aaa".to_string(),
            id2: "bbb".to_string(),
            similarity: 0.9,
        },
        DuplicatePair {
            id1: "ccc".to_string(),
            id2: "ddd".to_string(),
            similarity: 0.8,
        },
    ];

    let mut dismissed = std::collections::HashSet::new();
    dismissed.insert(normalize_pair_key("aaa", "bbb"));

    let filtered = filter_dismissed(pairs, &dismissed);

    assert_eq!(
        filtered.len(),
        1,
        "filter_dismissed must exclude dismissed pairs"
    );
    assert_eq!(
        filtered[0].id1, "ccc",
        "remaining pair must be the non-dismissed one"
    );
}

#[test]
fn contract_normalize_pair_key_is_commutative() {
    use vauchi_core::contact::merge::normalize_pair_key;

    let (a1, b1) = normalize_pair_key("xxx", "yyy");
    let (a2, b2) = normalize_pair_key("yyy", "xxx");

    assert_eq!(a1, a2, "normalize_pair_key must be commutative (id1)");
    assert_eq!(b1, b2, "normalize_pair_key must be commutative (id2)");
    assert!(a1 <= b1, "normalized id1 must be <= id2 lexicographically");
}

// ============================================================
// Contract: Contact Limit API (SP-12a)
// ============================================================

#[test]
fn contract_storage_get_contact_limit_has_default() {
    let wb = setup();
    let limit = wb
        .storage()
        .get_contact_limit()
        .expect("get_contact_limit must return a default");
    assert_eq!(limit, 10_000, "default contact limit must be 10,000");
}

#[test]
fn contract_storage_set_and_get_contact_limit() {
    let wb = setup();
    wb.storage()
        .set_contact_limit(500)
        .expect("set_contact_limit must accept usize");

    let limit = wb.storage().get_contact_limit().unwrap();
    assert_eq!(
        limit, 500,
        "get_contact_limit must return the value that was set"
    );
}

// ============================================================
// Contract: Dismissed Duplicates API (SP-12a)
// ============================================================

#[test]
fn contract_storage_dismiss_and_load_duplicates() {
    let wb = setup();

    // Initially no dismissed duplicates
    let dismissed = wb
        .storage()
        .load_dismissed_duplicates()
        .expect("load_dismissed_duplicates must return HashSet");
    assert!(
        dismissed.is_empty(),
        "initially there should be no dismissed duplicates"
    );

    // Dismiss a pair
    wb.storage()
        .dismiss_duplicate("aaa", "bbb")
        .expect("dismiss_duplicate must accept two IDs");

    let dismissed = wb.storage().load_dismissed_duplicates().unwrap();
    assert_eq!(
        dismissed.len(),
        1,
        "dismissed set must contain the dismissed pair"
    );

    // Pair should be normalized (aaa < bbb)
    assert!(
        dismissed.contains(&("aaa".to_string(), "bbb".to_string())),
        "dismissed pair must be stored normalized"
    );
}

#[test]
fn contract_storage_dismiss_is_order_independent() {
    let wb = setup();

    // Dismiss (bbb, aaa) — should normalize to (aaa, bbb)
    wb.storage().dismiss_duplicate("bbb", "aaa").unwrap();

    let dismissed = wb.storage().load_dismissed_duplicates().unwrap();
    assert!(
        dismissed.contains(&("aaa".to_string(), "bbb".to_string())),
        "dismiss_duplicate must normalize pair order"
    );
}

#[test]
fn contract_storage_undismiss_duplicate() {
    let wb = setup();

    wb.storage().dismiss_duplicate("aaa", "bbb").unwrap();
    assert_eq!(wb.storage().load_dismissed_duplicates().unwrap().len(), 1);

    wb.storage()
        .undismiss_duplicate("aaa", "bbb")
        .expect("undismiss_duplicate must accept two IDs");

    let dismissed = wb.storage().load_dismissed_duplicates().unwrap();
    assert!(
        dismissed.is_empty(),
        "undismiss_duplicate must remove the dismissed pair"
    );
}

// ============================================================
// Contract: Vauchi storage() accessor (SP-12a)
// ============================================================

#[test]
fn contract_vauchi_storage_accessor_exists() {
    // allow(zero_assertions): Compile-time shape check — fails to compile if accessor removed
    let wb = setup();
    // Verify that storage() returns a reference to Storage
    let _storage: &vauchi_core::Storage = wb.storage();
}
