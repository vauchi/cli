// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;

use crate::commands::common::open_vauchi_authenticated;
use crate::config::CliConfig;
use crate::display;
use crate::ui::invocation::present;
use vauchi_app::i18n::Locale;
use vauchi_app::ui::invocation::{Invocation, InvocationOutput, invoke};

/// Lists contacts through Core's one-shot invocation (ADR-066 Amendment
/// 2026-09-26 (b)); the authenticated handle carries the auth mode, so a
/// duress PIN lists decoys. Returns the process exit code.
pub fn list(
    config: &CliConfig,
    pin: Option<&str>,
    offset: usize,
    limit: usize,
    locale: &str,
) -> Result<u8> {
    let wb = open_vauchi_authenticated(config, pin)?;
    let output = if config.raw {
        InvocationOutput::Document
    } else {
        InvocationOutput::Text
    };
    let commands = invoke(
        &wb,
        &Invocation::ContactsList { offset, limit },
        output,
        Locale::from_code(locale).unwrap_or_default(),
    );
    Ok(present(
        &commands,
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    ))
}

/// Searches contacts by query (respects auth mode).
pub fn search(config: &CliConfig, pin: Option<&str>, query: &str, locale: &str) -> Result<()> {
    let wb = open_vauchi_authenticated(config, pin)?;
    let results = wb.search_contacts(query)?;

    if results.is_empty() {
        display::info(&format!("No contacts matching '{}'", query));
        return Ok(());
    }

    println!();
    println!(
        "{}",
        display::tf("cli.contacts.search.header", locale, &[("query", query)])
    );
    println!();

    for (i, contact) in results.iter().enumerate() {
        display::display_contact_summary(contact, i + 1);
    }

    println!();

    Ok(())
}
