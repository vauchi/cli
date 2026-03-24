// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! CLI argument definitions (clap structs and enums).

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use clap_complete::Shell;

#[derive(Parser)]
#[command(name = "vauchi")]
#[command(version, about = "Privacy-focused contact card exchange")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Data directory (default: ~/.vauchi)
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,

    /// Relay server URL
    #[arg(
        long,
        global = true,
        env = "VAUCHI_RELAY_URL",
        default_value = "wss://relay.vauchi.app"
    )]
    pub relay: String,

    /// Locale for output messages (en, de, fr, es)
    #[arg(long, global = true, env = "VAUCHI_LOCALE", default_value = "en")]
    pub locale: String,

    /// PIN for authentication (required when app password is configured)
    #[arg(long, global = true, env = "VAUCHI_PIN")]
    pub pin: Option<String>,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Create a new identity
    Init {
        /// Your display name
        name: String,
        /// Overwrite existing identity (destructive)
        #[arg(long)]
        force: bool,
    },

    /// Manage your contact card
    #[command(subcommand)]
    Card(CardCommands),

    /// Exchange contacts with another user
    #[command(subcommand)]
    Exchange(ExchangeCommands),

    /// Manage your contacts
    #[command(subcommand)]
    Contacts(ContactCommands),

    /// Social network utilities
    #[command(subcommand)]
    Social(SocialCommands),

    /// Manage linked devices
    #[command(subcommand)]
    Device(DeviceCommands),

    /// Manage visibility labels
    #[command(subcommand)]
    Labels(LabelCommands),

    /// Contact recovery via social vouching
    #[command(subcommand)]
    Recovery(RecoveryCommands),

    /// Message delivery management
    #[command(subcommand)]
    Delivery(DeliveryCommands),

    /// Sync with the relay server
    Sync,

    /// Export identity backup
    Export {
        /// Output file path
        output: PathBuf,
    },

    /// Import identity from backup
    Import {
        /// Input file path
        input: PathBuf,
    },

    /// Generate shell completions
    Completions {
        /// Shell type
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Privacy & data management (GDPR)
    #[command(subcommand)]
    Gdpr(GdprCommands),

    /// Duress PIN for plausible deniability
    #[command(subcommand)]
    Duress(DuressCommands),

    /// Emergency broadcast to trusted contacts
    #[command(subcommand)]
    Emergency(EmergencyCommands),

    /// Display FAQ and help information
    #[command(subcommand)]
    Faq(FaqCommands),

    /// Show how to support Vauchi
    SupportUs,

    /// Transport diagnostics and debugging tools
    #[command(subcommand)]
    Diag(crate::commands::diag::DiagCommands),

    /// Interactive onboarding flow
    Onboarding,
}

#[derive(Subcommand)]
pub(crate) enum DeliveryCommands {
    /// Show delivery status (record counts, retries, queue state)
    Status,

    /// List delivery records
    List {
        /// Filter by status: failed, pending, or all (default)
        #[arg(long)]
        status: Option<String>,
    },

    /// Process due delivery retries
    Retry,

    /// Run delivery cleanup (expire old records, remove terminal records)
    Cleanup,

    /// Translate a failure reason to a user-friendly message
    Translate {
        /// Failure reason code (e.g. connection_timeout, key_mismatch)
        reason: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum DuressCommands {
    /// Set up duress PIN (prompts for app password first if not set)
    Setup,

    /// Show duress status and configuration
    Status,

    /// Disable duress PIN
    Disable,

    /// Test authentication (shows Normal/Duress result, prompts for PIN)
    Test,
}

#[derive(Subcommand)]
pub(crate) enum EmergencyCommands {
    /// Configure trusted contacts and alert message
    Configure,

    /// Send emergency broadcast to all trusted contacts
    Send,

    /// Show emergency broadcast configuration
    Status,

    /// Disable emergency broadcast
    Disable,
}

#[derive(Subcommand)]
pub(crate) enum FaqCommands {
    /// List all FAQ items (optionally filter by search query)
    List {
        /// Search query to filter FAQs
        query: Option<String>,
    },

    /// Show FAQ categories
    Categories,

    /// Show FAQs in a specific category
    Category {
        /// Category: getting-started, privacy, recovery, contacts, updates, features
        name: String,
    },

    /// Show a specific FAQ by ID
    Show {
        /// FAQ ID (e.g., faq-phone-lost)
        id: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum GdprCommands {
    /// Export all personal data as JSON (optionally encrypted)
    Export {
        /// Output file path
        output: PathBuf,
        /// Encrypt export (prompts for password interactively)
        #[arg(long)]
        encrypt: bool,
        /// Password for encryption (prefer --encrypt for interactive prompt;
        /// kept for non-interactive/scripted use via env var VAUCHI_EXPORT_PASSWORD)
        #[arg(long, env = "VAUCHI_EXPORT_PASSWORD", hide = true)]
        password: Option<String>,
    },

    /// Schedule identity deletion (7-day grace period)
    ScheduleDeletion,

    /// Cancel a scheduled identity deletion
    CancelDeletion,

    /// Execute a scheduled identity deletion (after grace period)
    ExecuteDeletion,

    /// Emergency immediate deletion — no grace period
    PanicShred,

    /// Show current deletion status
    DeletionStatus,

    /// Show consent records
    ConsentStatus,

    /// Grant consent for a type (data_processing, contact_sharing, recovery_vouching)
    GrantConsent {
        /// Consent type
        consent_type: String,
    },

    /// Revoke consent for a type
    RevokeConsent {
        /// Consent type
        consent_type: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum CardCommands {
    /// Show your contact card
    Show,

    /// Add a field to your card
    ///
    /// For social fields, omit label and value to interactively select a
    /// network from the registry and enter a username.
    Add {
        /// Field type (email, phone, website, address, social, other)
        #[arg(value_name = "TYPE")]
        field_type: String,

        /// Field label (e.g., "work", "personal", "mobile"; optional for social)
        label: Option<String>,

        /// Field value (optional for social — prompts interactively)
        value: Option<String>,
    },

    /// Remove a field from your card
    Remove {
        /// Field label to remove
        label: String,
    },

    /// Edit a field value
    Edit {
        /// Field label to edit
        label: String,

        /// New value
        value: String,
    },

    /// Edit your display name
    EditName {
        /// New display name
        name: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum ExchangeCommands {
    /// Generate QR code for contact exchange
    Start,

    /// Complete exchange with another user's data
    Complete {
        /// Exchange data (wb:// URL or base64)
        data: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum ContactCommands {
    /// List all contacts
    List {
        /// Start offset for pagination
        #[arg(long, default_value = "0")]
        offset: usize,

        /// Maximum number of contacts to show (0 = all)
        #[arg(long, default_value = "0")]
        limit: usize,
    },

    /// Show contact details
    Show {
        /// Contact ID or name
        id: String,
    },

    /// Search contacts by name
    Search {
        /// Search query
        query: String,
    },

    /// Remove a contact
    Remove {
        /// Contact ID
        id: String,
    },

    /// Mark contact fingerprint as verified
    Verify {
        /// Contact ID
        id: String,
    },

    /// Hide a field from a contact
    Hide {
        /// Contact ID or name
        contact: String,
        /// Field label to hide
        field: String,
    },

    /// Show a field to a contact (make visible)
    Unhide {
        /// Contact ID or name
        contact: String,
        /// Field label to unhide
        field: String,
    },

    /// Show visibility rules for a contact
    Visibility {
        /// Contact ID or name
        contact: String,
    },

    /// Open a contact field in external app
    Open {
        /// Contact ID or name
        contact: String,
        /// Field label to open (optional - interactive if not specified)
        field: Option<String>,
    },

    /// Validate a contact's field (social proof)
    Validate {
        /// Contact ID or name
        contact: String,
        /// Field label to validate
        field: String,
    },

    /// Revoke your validation of a contact's field
    RevokeValidation {
        /// Contact ID or name
        contact: String,
        /// Field label to revoke validation for
        field: String,
    },

    /// Show validation status for a contact's fields
    ValidationStatus {
        /// Contact ID or name
        contact: String,
    },

    /// Mark a contact as trusted for recovery
    Trust {
        /// Contact ID or name
        id: String,
    },

    /// Remove recovery trust from a contact
    Untrust {
        /// Contact ID or name
        id: String,
    },

    /// Hide a contact from the default contact list
    HideContact {
        /// Contact ID or name
        id: String,
    },

    /// Unhide a previously hidden contact
    UnhideContact {
        /// Contact ID or name
        id: String,
    },

    /// List hidden contacts
    ListHidden,

    /// Block a contact (stops updates in both directions)
    Block {
        /// Contact ID or name
        id: String,
    },

    /// Unblock a previously blocked contact
    Unblock {
        /// Contact ID or name
        id: String,
    },

    /// List all blocked contacts
    ListBlocked,

    /// Mark a contact as a favorite
    Favorite {
        /// Contact ID or name
        id: String,
    },

    /// Remove a contact from favorites
    Unfavorite {
        /// Contact ID or name
        id: String,
    },

    /// Export a contact as vCard
    Export {
        /// Contact ID or name
        id: String,

        /// Output file path (e.g., contact.vcf)
        output: PathBuf,
    },

    /// Add a personal note to a contact
    AddNote {
        /// Contact ID or name
        id: String,

        /// Note text
        note: String,
    },

    /// Show personal note for a contact
    ShowNote {
        /// Contact ID or name
        id: String,
    },

    /// Edit personal note for a contact
    EditNote {
        /// Contact ID or name
        id: String,

        /// New note text
        note: String,
    },

    /// Delete personal note for a contact
    DeleteNote {
        /// Contact ID or name
        id: String,
    },

    /// Merge two contacts into one
    ///
    /// The first contact is the primary (keeps its name). Unique fields
    /// from the second contact are added, then the second contact is removed.
    Merge {
        /// Primary contact (ID or name) — keeps this contact's name
        contact1: String,
        /// Secondary contact (ID or name) — unique fields added, then removed
        contact2: String,
    },

    /// List potential duplicate contacts
    ///
    /// Shows contacts with high similarity scores. Previously dismissed
    /// false positives are excluded.
    Duplicates,

    /// Dismiss a duplicate pair as a false positive
    DismissDuplicate {
        /// First contact (ID or name)
        contact1: String,
        /// Second contact (ID or name)
        contact2: String,
    },

    /// Undo dismissal of a duplicate pair
    UndismissDuplicate {
        /// First contact (ID or name)
        contact1: String,
        /// Second contact (ID or name)
        contact2: String,
    },

    /// Show or set the contact limit
    ///
    /// Without --set, shows current limit and usage.
    /// With --set N, updates the maximum number of contacts.
    Limit {
        /// Set the contact limit to this value
        #[arg(long)]
        set: Option<usize>,
    },
}

#[derive(Subcommand)]
pub(crate) enum SocialCommands {
    /// List available social networks
    List {
        /// Optional search query
        query: Option<String>,
    },

    /// Get profile URL for a social network
    Url {
        /// Social network (e.g., twitter, github)
        network: String,
        /// Username on that network
        username: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum DeviceCommands {
    /// List all linked devices
    List,

    /// Show info about the current device
    Info,

    /// Generate QR code to link a new device
    Link,

    /// Join an existing identity (on new device)
    Join {
        /// QR data from existing device
        qr_data: String,

        /// Device name (skips interactive prompt)
        #[arg(long)]
        device_name: Option<String>,

        /// Skip confirmation prompts
        #[arg(long, short = 'y')]
        yes: bool,
    },

    /// Complete device linking (on existing device)
    Complete {
        /// Request data from new device
        request: String,
    },

    /// Finish device join (on new device)
    Finish {
        /// Response data from existing device
        response: String,
    },

    /// Revoke a linked device
    Revoke {
        /// Device ID prefix
        device_id: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum LabelCommands {
    /// List all labels
    List,

    /// Create a new label
    Create {
        /// Label name
        name: String,
    },

    /// Show label details
    Show {
        /// Label name or ID prefix
        label: String,
    },

    /// Rename a label
    Rename {
        /// Label name or ID prefix
        label: String,
        /// New name
        new_name: String,
    },

    /// Delete a label
    Delete {
        /// Label name or ID prefix
        label: String,
    },

    /// Add a contact to a label
    AddContact {
        /// Label name or ID prefix
        label: String,
        /// Contact name or ID prefix
        contact: String,
    },

    /// Remove a contact from a label
    RemoveContact {
        /// Label name or ID prefix
        label: String,
        /// Contact name or ID prefix
        contact: String,
    },

    /// Show a field to contacts in a label
    ShowField {
        /// Label name or ID prefix
        label: String,
        /// Field label
        field: String,
    },

    /// Hide a field from contacts in a label
    HideField {
        /// Label name or ID prefix
        label: String,
        /// Field label
        field: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum RecoveryCommands {
    /// Create a recovery claim for a lost identity
    Claim {
        /// Old public key (hex) from lost device
        old_pk: String,
    },

    /// Vouch for someone's recovery claim
    Vouch {
        /// Recovery claim data (base64)
        claim: String,

        /// Skip interactive confirmation (for automated/E2E testing)
        #[arg(long)]
        yes: bool,
    },

    /// Add a voucher to your recovery proof
    AddVoucher {
        /// Voucher data (base64)
        voucher: String,
    },

    /// Show recovery status
    Status,

    /// Show completed recovery proof
    Proof,

    /// Verify a recovery proof from a contact
    Verify {
        /// Recovery proof data (base64)
        proof: String,
    },

    /// Manage recovery settings
    #[command(subcommand)]
    Settings(RecoverySettingsCommands),
}

#[derive(Subcommand)]
pub(crate) enum RecoverySettingsCommands {
    /// Show current settings
    Show,

    /// Set recovery thresholds
    Set {
        /// Vouchers required for recovery (1-10)
        #[arg(long, default_value = "3")]
        recovery: u32,

        /// Mutual contacts for high confidence (1-recovery)
        #[arg(long, default_value = "2")]
        verification: u32,
    },
}
