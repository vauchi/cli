// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! `vauchi exchange link`: the remote exchange the apps' Link mode offers
//! (private#670), through Core's link-exchange API: start or accept, then
//! advance until it completes or the wait runs out.

use std::io::IsTerminal;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use vauchi_core::api::LinkExchangeProgress;
use vauchi_core::exchange::link_mode::parse_link_url;

use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

/// Pause between advances, so a deposit by the other side is seen within a
/// few seconds.
const POLL: Duration = Duration::from_secs(2);

/// Shares this device's link (`their_link` absent) or accepts the other
/// side's after consent (`yes`, or a prompt at a terminal), then waits up to
/// `wait` for the exchange to complete.
pub fn link(config: &CliConfig, their_link: Option<&str>, wait: Duration, yes: bool) -> Result<()> {
    let their_link = their_link.map(str::trim);
    let parsed = match their_link {
        Some(link) => match parse_link_url(link) {
            Some(parsed) => Some(parsed),
            None => bail!("not a Vauchi exchange link: {link:?}"),
        },
        None => None,
    };

    let vauchi = open_vauchi(config)?;
    let mut exchange = match (their_link, parsed) {
        (Some(link), Some(parsed)) => {
            // A link is never processed without consent (ADR-021); the name
            // in it is the sender's claim, not yet checked.
            let sender = parsed.sender_name.as_deref().unwrap_or("someone");
            display::info(&format!(
                "Exchange request from {sender} (name not verified). Only accept if you trust the source."
            ));
            if !consented(yes)? {
                display::info("Exchange declined.");
                return Ok(());
            }
            vauchi.accept_link_exchange(link)?
        }
        _ => {
            let (url, exchange) = vauchi.start_link_exchange()?;
            display::info("Send this link to the other person:");
            println!("{url}");
            exchange
        }
    };

    let deadline = Instant::now() + wait;
    loop {
        match vauchi.advance_link_exchange(&mut exchange)? {
            LinkExchangeProgress::Completed { .. } => {
                display::success("Exchange complete.");
                return Ok(());
            }
            LinkExchangeProgress::Failed { reason } => {
                bail!("link exchange failed: {reason}");
            }
            LinkExchangeProgress::Waiting => {}
        }
        let now = Instant::now();
        if now >= deadline {
            bail!("link exchange not completed within {} s", wait.as_secs());
        }
        std::thread::sleep(POLL.min(deadline - now));
    }
}

/// `--yes`, or a typed "yes" at a terminal.
fn consented(yes: bool) -> Result<bool> {
    if yes {
        return Ok(true);
    }
    if !std::io::stdin().is_terminal() {
        bail!("run again with --yes to accept this link without a prompt");
    }
    let answer: String = dialoguer::Input::new()
        .with_prompt("Type 'yes' to accept this exchange")
        .interact_text()?;
    Ok(answer.trim().eq_ignore_ascii_case("yes"))
}
