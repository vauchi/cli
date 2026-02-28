// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Vauchi CLI
//!
//! Command-line interface for Vauchi - privacy-focused contact card exchange.

mod commands;
mod config;
mod display;
mod protocol;

use std::path::PathBuf;

use std::io;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};

use config::CliConfig;

#[derive(Parser)]
#[command(name = "vauchi")]
#[command(version, about = "Privacy-focused contact card exchange")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Data directory (default: ~/.vauchi)
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,

    /// Relay server URL
    #[arg(
        long,
        global = true,
        env = "VAUCHI_RELAY_URL",
        default_value = "wss://relay.vauchi.app"
    )]
    relay: String,

    /// Locale for output messages (en, de, fr, es)
    #[arg(long, global = true, env = "VAUCHI_LOCALE", default_value = "en")]
    locale: String,
}

#[derive(Subcommand)]
enum Commands {
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

    /// Tor privacy mode configuration
    #[command(subcommand)]
    Tor(TorCommands),

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
}

#[derive(Subcommand)]
enum DeliveryCommands {
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
enum DuressCommands {
    /// Set up duress PIN (prompts for app password first if not set)
    Setup,

    /// Show duress status and configuration
    Status,

    /// Disable duress PIN
    Disable,

    /// Test authentication (shows Normal/Duress result)
    Test {
        /// PIN to test
        pin: String,
    },
}

#[derive(Subcommand)]
enum EmergencyCommands {
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
enum FaqCommands {
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
enum TorCommands {
    /// Enable Tor mode
    Enable,

    /// Disable Tor mode
    Disable,

    /// Show Tor status
    Status,

    /// Request a new Tor circuit
    NewCircuit,

    /// Manage bridge addresses
    #[command(subcommand)]
    Bridges(TorBridgesCommands),
}

#[derive(Subcommand)]
enum TorBridgesCommands {
    /// Add a bridge address
    Add {
        /// Bridge address (e.g., "obfs4 192.168.1.1:443")
        addr: String,
    },

    /// List configured bridges
    List,

    /// Clear all bridges
    Clear,
}

#[derive(Subcommand)]
enum GdprCommands {
    /// Export all personal data as JSON (encrypted by default when --password is given)
    Export {
        /// Output file path
        output: PathBuf,
        /// Encrypt export with this password
        #[arg(long)]
        password: Option<String>,
    },

    /// Schedule account deletion (7-day grace period)
    ScheduleDeletion,

    /// Cancel a scheduled account deletion
    CancelDeletion,

    /// Execute a scheduled account deletion (after grace period)
    ExecuteDeletion,

    /// Emergency immediate deletion — no grace period
    PanicShred,

    /// Show current deletion status
    DeletionStatus,

    /// Show consent records
    ConsentStatus,

    /// Grant consent for a type (data_processing, contact_sharing, analytics, recovery_vouching)
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
enum CardCommands {
    /// Show your contact card
    Show,

    /// Add a field to your card
    Add {
        /// Field type (email, phone, website, address, social, other)
        #[arg(value_name = "TYPE")]
        field_type: String,

        /// Field label (e.g., "work", "personal", "mobile")
        label: String,

        /// Field value
        value: String,
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
enum ExchangeCommands {
    /// Generate QR code for contact exchange
    Start,

    /// Complete exchange with another user's data
    Complete {
        /// Exchange data (wb:// URL or base64)
        data: String,
    },
}

#[derive(Subcommand)]
enum ContactCommands {
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
}

#[derive(Subcommand)]
enum SocialCommands {
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
enum DeviceCommands {
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
enum LabelCommands {
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
enum RecoveryCommands {
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
enum RecoverySettingsCommands {
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Resolve data directory
    let data_dir = cli.data_dir.unwrap_or_else(|| {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("vauchi")
    });

    let config = CliConfig {
        data_dir,
        relay_url: cli.relay,
    };

    match cli.command {
        Commands::Init { name, force } => {
            commands::init::run(&name, force, &config)?;
        }
        Commands::Card(cmd) => match cmd {
            CardCommands::Show => commands::card::show(&config)?,
            CardCommands::Add {
                field_type,
                label,
                value,
            } => {
                commands::card::add(&config, &field_type, &label, &value)?;
            }
            CardCommands::Remove { label } => {
                commands::card::remove(&config, &label)?;
            }
            CardCommands::Edit { label, value } => {
                commands::card::edit(&config, &label, &value)?;
            }
            CardCommands::EditName { name } => {
                commands::card::edit_name(&config, &name)?;
            }
        },
        Commands::Exchange(cmd) => match cmd {
            ExchangeCommands::Start => commands::exchange::start(&config)?,
            ExchangeCommands::Complete { data } => {
                commands::exchange::complete(&config, &data).await?;
            }
        },
        Commands::Contacts(cmd) => match cmd {
            ContactCommands::List { offset, limit } => {
                commands::contacts::list(&config, offset, limit)?
            }
            ContactCommands::Show { id } => commands::contacts::show(&config, &id)?,
            ContactCommands::Search { query } => commands::contacts::search(&config, &query)?,
            ContactCommands::Remove { id } => commands::contacts::remove(&config, &id)?,
            ContactCommands::Verify { id } => commands::contacts::verify(&config, &id)?,
            ContactCommands::Hide { contact, field } => {
                commands::contacts::hide_field(&config, &contact, &field)?;
            }
            ContactCommands::Unhide { contact, field } => {
                commands::contacts::unhide_field(&config, &contact, &field)?;
            }
            ContactCommands::Visibility { contact } => {
                commands::contacts::show_visibility(&config, &contact)?;
            }
            ContactCommands::Open { contact, field } => {
                if let Some(field_label) = field {
                    commands::contacts::open_field(&config, &contact, &field_label)?;
                } else {
                    commands::contacts::open_interactive(&config, &contact)?;
                }
            }
            ContactCommands::Validate { contact, field } => {
                commands::contacts::validate_field(&config, &contact, &field)?;
            }
            ContactCommands::RevokeValidation { contact, field } => {
                commands::contacts::revoke_validation(&config, &contact, &field)?;
            }
            ContactCommands::ValidationStatus { contact } => {
                commands::contacts::show_validation_status(&config, &contact)?;
            }
            ContactCommands::Trust { id } => {
                commands::contacts::trust(&config, &id)?;
            }
            ContactCommands::Untrust { id } => {
                commands::contacts::untrust(&config, &id)?;
            }
        },
        Commands::Social(cmd) => match cmd {
            SocialCommands::List { query } => {
                display::display_social_networks(query.as_deref());
            }
            SocialCommands::Url { network, username } => {
                use vauchi_core::SocialNetworkRegistry;
                let registry = SocialNetworkRegistry::with_defaults();
                match registry.profile_url(&network, &username) {
                    Some(url) => println!("{}", url),
                    None => {
                        display::warning(&format!("Unknown network: {}", network));
                        display::info("Use 'vauchi social list' to see available networks");
                    }
                }
            }
        },
        Commands::Device(cmd) => match cmd {
            DeviceCommands::List => commands::device::list(&config)?,
            DeviceCommands::Info => commands::device::info(&config)?,
            DeviceCommands::Link => commands::device::link(&config)?,
            DeviceCommands::Join {
                qr_data,
                device_name,
                yes,
            } => commands::device::join(&config, &qr_data, device_name.as_deref(), yes)?,
            DeviceCommands::Complete { request } => commands::device::complete(&config, &request)?,
            DeviceCommands::Finish { response } => commands::device::finish(&config, &response)?,
            DeviceCommands::Revoke { device_id } => commands::device::revoke(&config, &device_id)?,
        },
        Commands::Labels(cmd) => match cmd {
            LabelCommands::List => commands::labels::list(&config)?,
            LabelCommands::Create { name } => commands::labels::create(&config, &name)?,
            LabelCommands::Show { label } => commands::labels::show(&config, &label)?,
            LabelCommands::Rename { label, new_name } => {
                commands::labels::rename(&config, &label, &new_name)?
            }
            LabelCommands::Delete { label } => commands::labels::delete(&config, &label)?,
            LabelCommands::AddContact { label, contact } => {
                commands::labels::add_contact(&config, &label, &contact)?
            }
            LabelCommands::RemoveContact { label, contact } => {
                commands::labels::remove_contact(&config, &label, &contact)?
            }
            LabelCommands::ShowField { label, field } => {
                commands::labels::show_field(&config, &label, &field)?
            }
            LabelCommands::HideField { label, field } => {
                commands::labels::hide_field(&config, &label, &field)?
            }
        },
        Commands::Recovery(cmd) => match cmd {
            RecoveryCommands::Claim { old_pk } => commands::recovery::claim(&config, &old_pk)?,
            RecoveryCommands::Vouch { claim, yes } => {
                commands::recovery::vouch(&config, &claim, yes)?
            }
            RecoveryCommands::AddVoucher { voucher } => {
                commands::recovery::add_voucher(&config, &voucher)?
            }
            RecoveryCommands::Status => commands::recovery::status(&config)?,
            RecoveryCommands::Proof => commands::recovery::proof_show(&config)?,
            RecoveryCommands::Verify { proof } => commands::recovery::verify(&config, &proof)?,
            RecoveryCommands::Settings(settings_cmd) => match settings_cmd {
                RecoverySettingsCommands::Show => commands::recovery::settings_show(&config)?,
                RecoverySettingsCommands::Set {
                    recovery,
                    verification,
                } => {
                    commands::recovery::settings_set(&config, recovery, verification)?;
                }
            },
        },
        Commands::Delivery(cmd) => match cmd {
            DeliveryCommands::Status => commands::delivery::status(&config)?,
            DeliveryCommands::List { status } => {
                commands::delivery::list(&config, status.as_deref())?
            }
            DeliveryCommands::Retry => commands::delivery::retry(&config)?,
            DeliveryCommands::Cleanup => commands::delivery::cleanup(&config)?,
            DeliveryCommands::Translate { reason } => commands::delivery::translate(&reason)?,
        },
        Commands::Sync => {
            commands::sync::run(&config).await?;
        }
        Commands::Export { output } => {
            commands::backup::export(&config, &output)?;
        }
        Commands::Import { input } => {
            commands::backup::import(&config, &input)?;
        }
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "vauchi", &mut io::stdout());
        }
        Commands::Gdpr(cmd) => match cmd {
            GdprCommands::Export { output, password } => {
                commands::gdpr::export_data(&config, &output, password.as_deref())?;
            }
            GdprCommands::ExecuteDeletion => {
                commands::gdpr::execute_deletion(&config).await?;
            }
            GdprCommands::PanicShred => {
                commands::gdpr::panic_shred(&config).await?;
            }
            GdprCommands::ScheduleDeletion => {
                commands::gdpr::schedule_deletion(&config)?;
            }
            GdprCommands::CancelDeletion => {
                commands::gdpr::cancel_deletion(&config)?;
            }
            GdprCommands::DeletionStatus => {
                commands::gdpr::deletion_status(&config)?;
            }
            GdprCommands::ConsentStatus => {
                commands::gdpr::consent_status(&config)?;
            }
            GdprCommands::GrantConsent { consent_type } => {
                commands::gdpr::grant_consent(&config, &consent_type)?;
            }
            GdprCommands::RevokeConsent { consent_type } => {
                commands::gdpr::revoke_consent(&config, &consent_type)?;
            }
        },
        Commands::Tor(cmd) => match cmd {
            TorCommands::Enable => commands::tor::enable(&config)?,
            TorCommands::Disable => commands::tor::disable(&config)?,
            TorCommands::Status => commands::tor::status(&config)?,
            TorCommands::NewCircuit => commands::tor::new_circuit(&config)?,
            TorCommands::Bridges(bridges_cmd) => match bridges_cmd {
                TorBridgesCommands::Add { addr } => {
                    commands::tor::bridges_add(&config, &addr)?;
                }
                TorBridgesCommands::List => commands::tor::bridges_list(&config)?,
                TorBridgesCommands::Clear => commands::tor::bridges_clear(&config)?,
            },
        },
        Commands::Duress(cmd) => match cmd {
            DuressCommands::Setup => commands::duress::setup(&config)?,
            DuressCommands::Status => commands::duress::status(&config)?,
            DuressCommands::Disable => commands::duress::disable(&config)?,
            DuressCommands::Test { pin } => commands::duress::test(&config, &pin)?,
        },
        Commands::Emergency(cmd) => match cmd {
            EmergencyCommands::Configure => commands::emergency::configure(&config)?,
            EmergencyCommands::Send => commands::emergency::send(&config)?,
            EmergencyCommands::Status => commands::emergency::status(&config)?,
            EmergencyCommands::Disable => commands::emergency::disable(&config)?,
        },
        Commands::Faq(cmd) => match cmd {
            FaqCommands::List { query } => {
                display::display_faqs(query.as_deref(), &cli.locale);
            }
            FaqCommands::Categories => {
                display::display_faq_categories(&cli.locale);
            }
            FaqCommands::Category { name } => {
                display::display_faqs_by_category(&name, &cli.locale);
            }
            FaqCommands::Show { id } => {
                display::display_faq_by_id(&id, &cli.locale);
            }
        },
        Commands::SupportUs => commands::support::run(),
    }

    Ok(())
}
