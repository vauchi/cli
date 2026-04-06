// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::Result;

use crate::commands::common::open_vauchi_authenticated;
use crate::config::CliConfig;
use crate::display;

/// Lists all contacts (respects auth mode — duress PIN shows decoys).
pub fn list(config: &CliConfig, pin: Option<&str>, offset: usize, limit: usize) -> Result<()> {
    let wb = open_vauchi_authenticated(config, pin)?;
    let total = wb.contact_count().unwrap_or(0);

    if total == 0 {
        display::info("No contacts yet. Exchange with someone using:");
        println!("  vauchi exchange start");
        return Ok(());
    }

    // Use core pagination API instead of manual slice
    let paginated = offset > 0 || limit > 0;
    let contacts = if paginated {
        wb.list_contacts_paginated(offset, limit)?
    } else {
        wb.list_contacts()?
    };

    println!();
    if paginated {
        println!(
            "Contacts (showing {}-{} of {}):",
            offset + 1,
            offset + contacts.len(),
            total
        );
    } else {
        println!("Contacts ({}):", total);
    }
    println!();

    if config.raw {
        let json: Vec<_> = contacts.iter().map(crate::raw::ContactJson::from).collect();
        return crate::raw::print_json(&json);
    }

    display::display_contacts_table(&contacts);

    println!();

    Ok(())
}

/// Searches contacts by query (respects auth mode).
pub fn search(config: &CliConfig, pin: Option<&str>, query: &str) -> Result<()> {
    let wb = open_vauchi_authenticated(config, pin)?;
    let results = wb.search_contacts(query)?;

    if results.is_empty() {
        display::info(&format!("No contacts matching '{}'", query));
        return Ok(());
    }

    println!();
    println!("Search results for '{}':", query);
    println!();

    for (i, contact) in results.iter().enumerate() {
        display::display_contact_summary(contact, i + 1);
    }

    println!();

    Ok(())
}
