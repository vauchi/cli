// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Command dispatch — routes parsed CLI args to command handlers.

use std::io;

use anyhow::Result;
use clap::CommandFactory;
use clap_complete::generate;

use crate::args::*;
use crate::commands;
use crate::config::CliConfig;
use crate::display;

/// Dispatch a parsed [`Commands`] variant to the appropriate handler.
pub(crate) async fn run(
    command: Commands,
    config: &CliConfig,
    pin: Option<&str>,
    locale: &str,
) -> Result<()> {
    match command {
        Commands::Init { name, force } => {
            commands::init::run(&name, force, config)?;
        }
        Commands::Card(cmd) => match cmd {
            CardCommands::Show => commands::card::show(config)?,
            CardCommands::Add {
                field_type,
                label,
                value,
            } => {
                // Social fields support interactive prompting when label/value
                // are omitted: `vauchi card add social`
                let is_social = vauchi_core::FieldType::from_alias(&field_type)
                    .map(|(ft, _)| ft.is_social())
                    .unwrap_or(false);

                match (label, value) {
                    (Some(l), Some(v)) => {
                        commands::card::add(config, &field_type, &l, &v)?;
                    }
                    (None, None) if is_social => {
                        commands::card::add_social_interactive(config)?;
                    }
                    _ => {
                        if is_social {
                            display::info(
                                "Tip: run 'vauchi card add social' to select a network interactively",
                            );
                        }
                        anyhow::bail!(
                            "Missing required arguments. Usage: vauchi card add <TYPE> <LABEL> <VALUE>"
                        );
                    }
                }
            }
            CardCommands::Remove { label } => {
                commands::card::remove(config, &label)?;
            }
            CardCommands::Edit { label, value } => {
                commands::card::edit(config, &label, &value)?;
            }
            CardCommands::EditName { name } => {
                commands::card::edit_name(config, &name)?;
            }
        },
        Commands::Exchange(cmd) => match cmd {
            ExchangeCommands::Start => commands::exchange::start(config)?,
            ExchangeCommands::Complete { data } => {
                commands::exchange::complete(config, &data)?;
            }
            ExchangeCommands::Usb { address } => {
                commands::exchange::usb_exchange(config, &address)?;
            }
            ExchangeCommands::UsbListen { port } => {
                commands::exchange::usb_listen(config, port)?;
            }
        },
        Commands::Contacts(cmd) => match cmd {
            ContactCommands::List {
                offset,
                limit,
                archived,
            } => {
                if archived {
                    commands::contacts::list_archived(config)?;
                } else {
                    commands::contacts::list(config, pin, offset, limit)?;
                }
            }
            ContactCommands::Show { id } => commands::contacts::show(config, pin, &id)?,
            ContactCommands::Search { query } => commands::contacts::search(config, pin, &query)?,
            ContactCommands::Remove { id } => commands::contacts::remove(config, &id)?,
            ContactCommands::Verify { id } => commands::contacts::verify(config, &id)?,
            ContactCommands::Hide { contact, field } => {
                commands::contacts::hide_field(config, &contact, &field)?;
            }
            ContactCommands::Unhide { contact, field } => {
                commands::contacts::unhide_field(config, &contact, &field)?;
            }
            ContactCommands::Visibility { contact } => {
                commands::contacts::show_visibility(config, &contact)?;
            }
            ContactCommands::Open { contact, field } => {
                if let Some(field_label) = field {
                    commands::contacts::open_field(config, &contact, &field_label)?;
                } else {
                    commands::contacts::open_interactive(config, &contact)?;
                }
            }
            ContactCommands::Trust { id } => {
                commands::contacts::trust(config, &id)?;
            }
            ContactCommands::Untrust { id } => {
                commands::contacts::untrust(config, &id)?;
            }
            ContactCommands::HideContact { id } => {
                commands::contacts::hide_contact(config, &id)?;
            }
            ContactCommands::UnhideContact { id } => {
                commands::contacts::unhide_contact(config, &id)?;
            }
            ContactCommands::ListHidden => {
                commands::contacts::list_hidden(config)?;
            }
            ContactCommands::Block { id } => {
                commands::contacts::block(config, &id)?;
            }
            ContactCommands::Unblock { id } => {
                commands::contacts::unblock(config, &id)?;
            }
            ContactCommands::ListBlocked => {
                commands::contacts::list_blocked(config)?;
            }
            ContactCommands::Favorite { id } => {
                commands::contacts::favorite(config, &id)?;
            }
            ContactCommands::Unfavorite { id } => {
                commands::contacts::unfavorite(config, &id)?;
            }
            ContactCommands::Export { id, output } => {
                commands::contacts::export(config, &id, output.to_str().unwrap())?;
            }
            ContactCommands::ImportVcf { file } => {
                commands::contacts::import_vcf(config, &file)?;
            }
            ContactCommands::AddNote { id, note } => {
                commands::contacts::add_note(config, &id, &note)?;
            }
            ContactCommands::ShowNote { id } => {
                commands::contacts::show_note(config, &id)?;
            }
            ContactCommands::EditNote { id, note } => {
                commands::contacts::edit_note(config, &id, &note)?;
            }
            ContactCommands::DeleteNote { id } => {
                commands::contacts::delete_note(config, &id)?;
            }
            ContactCommands::Merge { contact1, contact2 } => {
                commands::contacts::merge(config, &contact1, &contact2)?;
            }
            ContactCommands::Duplicates => {
                commands::contacts::duplicates(config)?;
            }
            ContactCommands::DismissDuplicate { contact1, contact2 } => {
                commands::contacts::dismiss_duplicate(config, &contact1, &contact2)?;
            }
            ContactCommands::UndismissDuplicate { contact1, contact2 } => {
                commands::contacts::undismiss_duplicate(config, &contact1, &contact2)?;
            }
            ContactCommands::Limit { set } => {
                commands::contacts::limit(config, set)?;
            }
            ContactCommands::Delete { id, yes } => {
                commands::contacts::delete(config, &id, yes)?;
            }
            ContactCommands::Archive { id } => {
                commands::contacts::archive(config, &id)?;
            }
            ContactCommands::Unarchive { id } => {
                commands::contacts::unarchive(config, &id)?;
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
            DeviceCommands::List => commands::device::list(config)?,
            DeviceCommands::Info => commands::device::info(config)?,
            DeviceCommands::Link => commands::device::link(config)?,
            DeviceCommands::Join {
                qr_data,
                device_name,
                yes,
            } => commands::device::join(config, &qr_data, device_name.as_deref(), yes)?,
            DeviceCommands::Complete { request, yes } => {
                commands::device::complete(config, &request, yes)?
            }
            DeviceCommands::Finish { response } => commands::device::finish(config, &response)?,
            DeviceCommands::Revoke { device_id } => commands::device::revoke(config, &device_id)?,
            DeviceCommands::Replace(cmd) => match cmd {
                DeviceReplaceCommands::Setup => {
                    commands::device_replacement::run_setup()?;
                }
                DeviceReplaceCommands::Transfer => {
                    commands::device_replacement::run_transfer()?;
                }
                DeviceReplaceCommands::PostRestore => {
                    commands::device_replacement::run_post_restore()?;
                }
            },
        },
        Commands::Labels(cmd) => match cmd {
            LabelCommands::List => commands::labels::list(config)?,
            LabelCommands::Create { name } => commands::labels::create(config, &name)?,
            LabelCommands::Show { label } => commands::labels::show(config, &label)?,
            LabelCommands::Rename { label, new_name } => {
                commands::labels::rename(config, &label, &new_name)?
            }
            LabelCommands::Delete { label } => commands::labels::delete(config, &label)?,
            LabelCommands::AddContact { label, contact } => {
                commands::labels::add_contact(config, &label, &contact)?
            }
            LabelCommands::RemoveContact { label, contact } => {
                commands::labels::remove_contact(config, &label, &contact)?
            }
            LabelCommands::ShowField { label, field } => {
                commands::labels::show_field(config, &label, &field)?
            }
            LabelCommands::HideField { label, field } => {
                commands::labels::hide_field(config, &label, &field)?
            }
        },
        Commands::Recovery(cmd) => match cmd {
            RecoveryCommands::Claim { old_pk } => commands::recovery::claim(config, &old_pk)?,
            RecoveryCommands::Vouch { claim, yes } => {
                commands::recovery::vouch(config, &claim, yes)?
            }
            RecoveryCommands::AddVoucher { voucher } => {
                commands::recovery::add_voucher(config, &voucher)?
            }
            RecoveryCommands::Status => commands::recovery::status(config)?,
            RecoveryCommands::Proof => commands::recovery::proof_show(config)?,
            RecoveryCommands::Verify { proof } => commands::recovery::verify(config, &proof)?,
            RecoveryCommands::Settings(settings_cmd) => match settings_cmd {
                RecoverySettingsCommands::Show => commands::recovery::settings_show(config)?,
                RecoverySettingsCommands::Set {
                    recovery,
                    verification,
                } => {
                    commands::recovery::settings_set(config, recovery, verification)?;
                }
            },
        },
        Commands::Delivery(cmd) => match cmd {
            DeliveryCommands::Status => commands::delivery::status(config)?,
            DeliveryCommands::List { status } => {
                commands::delivery::list(config, status.as_deref())?
            }
            DeliveryCommands::Retry => commands::delivery::retry(config)?,
            DeliveryCommands::Cleanup => commands::delivery::cleanup(config)?,
            DeliveryCommands::Translate { reason } => commands::delivery::translate(&reason)?,
        },
        Commands::Sync => {
            commands::sync::run(config)?;
        }
        Commands::Activity { since } => {
            commands::activity::run(config, since.unwrap_or(60))?;
        }
        Commands::Export { output, full } => {
            if full {
                commands::backup::export_full(config, &output)?;
            } else {
                commands::backup::export(config, &output)?;
            }
        }
        Commands::Import { input, full } => {
            if full {
                commands::backup::import_full(config, &input)?;
            } else {
                commands::backup::import(config, &input)?;
            }
        }
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "vauchi", &mut io::stdout());
        }
        Commands::Gdpr(cmd) => match cmd {
            GdprCommands::Export {
                output,
                encrypt,
                password,
            } => {
                let password = if let Some(pw) = password {
                    // Hidden --password flag or env var (for scripted/test use)
                    Some(pw)
                } else if encrypt {
                    let pw = dialoguer::Password::new()
                        .with_prompt("Encryption password")
                        .with_confirmation("Confirm password", "Passwords don't match")
                        .interact()?;
                    Some(pw)
                } else {
                    None
                };
                commands::gdpr::export_data(config, &output, password.as_deref())?;
            }
            GdprCommands::ExecuteDeletion => {
                commands::gdpr::execute_deletion(config).await?;
            }
            GdprCommands::PanicShred => {
                commands::gdpr::panic_shred(config).await?;
            }
            GdprCommands::ScheduleDeletion => {
                commands::gdpr::schedule_deletion(config)?;
            }
            GdprCommands::CancelDeletion => {
                commands::gdpr::cancel_deletion(config)?;
            }
            GdprCommands::DeletionStatus => {
                commands::gdpr::deletion_status(config)?;
            }
            GdprCommands::ConsentStatus => {
                commands::gdpr::consent_status(config)?;
            }
            GdprCommands::GrantConsent { consent_type } => {
                commands::gdpr::grant_consent(config, &consent_type)?;
            }
            GdprCommands::RevokeConsent { consent_type } => {
                commands::gdpr::revoke_consent(config, &consent_type)?;
            }
        },
        Commands::Duress(cmd) => match cmd {
            DuressCommands::Setup => commands::duress::setup(config)?,
            DuressCommands::Status => commands::duress::status(config)?,
            DuressCommands::Disable => commands::duress::disable(config)?,
            DuressCommands::Test => {
                let pin_value = if let Some(p) = pin {
                    p.to_owned()
                } else {
                    dialoguer::Password::new()
                        .with_prompt("Enter PIN to test")
                        .interact()?
                };
                commands::duress::test(config, &pin_value)?;
            }
        },
        Commands::Emergency(cmd) => match cmd {
            EmergencyCommands::Configure => commands::emergency::configure(config)?,
            EmergencyCommands::Send => commands::emergency::send(config)?,
            EmergencyCommands::Status => commands::emergency::status(config)?,
            EmergencyCommands::Disable => commands::emergency::disable(config)?,
        },
        Commands::Faq(cmd) => match cmd {
            FaqCommands::List { query } => {
                display::display_faqs(query.as_deref(), locale);
            }
            FaqCommands::Categories => {
                display::display_faq_categories(locale);
            }
            FaqCommands::Category { name } => {
                display::display_faqs_by_category(&name, locale);
            }
            FaqCommands::Show { id } => {
                display::display_faq_by_id(&id, locale);
            }
        },
        Commands::SupportUs => commands::support::run(),
        Commands::Diag(cmd) => match cmd {
            commands::diag::DiagCommands::Transport => commands::diag::transport()?,
            commands::diag::DiagCommands::Trace { file } => commands::diag::trace(&file)?,
            commands::diag::DiagCommands::AnimatedQr(qr_cmd) => match qr_cmd {
                commands::diag::AnimatedQrCommands::Encode {
                    file,
                    fps,
                    chunk_size,
                } => commands::diag::animated_qr_encode(&file, fps, chunk_size)?,
            },
        },
        Commands::Onboarding => {
            commands::onboarding::run()?;
        }
    }

    Ok(())
}
