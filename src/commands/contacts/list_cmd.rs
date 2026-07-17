// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;

use crate::commands::common::open_vauchi_authenticated;
use crate::config::CliConfig;
use crate::display;

/// Lists all contacts (respects auth mode — duress PIN shows decoys).
pub fn list(
    config: &CliConfig,
    pin: Option<&str>,
    offset: usize,
    limit: usize,
    locale: &str,
) -> Result<()> {
    let wb = open_vauchi_authenticated(config, pin)?;
    let total = wb.contact_count().unwrap_or(0);

    if total == 0 {
        if config.raw {
            return crate::raw::print_json(&Vec::<crate::raw::ContactJson>::new());
        }
        display::info(&display::t("cli.contacts.list.no_contacts", locale));
        println!(
            "  {}",
            display::t("cli.contacts.list.exchange_command", locale)
        );
        return Ok(());
    }

    // Use core pagination API instead of manual slice
    let paginated = offset > 0 || limit > 0;
    let contacts = if paginated {
        wb.list_contacts_paginated(offset, limit)?
    } else {
        wb.list_contacts()?
    };

    if config.raw {
        let json: Vec<_> = contacts.iter().map(crate::raw::ContactJson::from).collect();
        return crate::raw::print_json(&json);
    }

    println!();
    if paginated {
        println!(
            "{}",
            display::tf(
                "cli.contacts.list.paginated_header",
                locale,
                &[
                    ("start", &(offset + 1).to_string()),
                    ("end", &(offset + contacts.len()).to_string()),
                    ("total", &total.to_string()),
                ]
            )
        );
    } else {
        println!(
            "{}",
            display::tf(
                "cli.contacts.list.header",
                locale,
                &[("count", &total.to_string())]
            )
        );
    }
    println!();

    display::display_contacts_table(&contacts);

    println!();

    Ok(())
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
