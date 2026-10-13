// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! `vauchi exchange link`: the remote exchange the apps' Link mode offers
//! (private#670). Core's `AppEngine` runs the whole ceremony, the relay
//! escrow included, exactly as it does for the TUI; this command only
//! starts it, prints the share link, and wakes the engine until the
//! exchange ends or the wait runs out.

use std::io::IsTerminal;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use vauchi_app::ui::{AppEngine, AppScreen, Component, ScreenModel, UserAction, WorkflowEngine};

use crate::commands::common::open_vauchi;
use crate::config::CliConfig;
use crate::display;

const SUCCEEDED: [&str; 2] = ["exchange_link_success", "link_responder_completed"];
const FAILED: [&str; 2] = ["exchange_link_failed", "link_responder_failed"];

/// Longest pause between wakeups, so a deposit by the other side is seen
/// within a few seconds.
const POLL: Duration = Duration::from_secs(2);

/// Shares this device's link (`their_link` absent) or accepts the other
/// side's after consent (`yes`, or a prompt at a terminal), then waits up to
/// `wait` for the exchange to complete.
pub fn link(config: &CliConfig, their_link: Option<&str>, wait: Duration, yes: bool) -> Result<()> {
    let their_link = their_link.map(str::trim);
    if let Some(link) = their_link
        && vauchi_core::exchange::link_mode::parse_link_url(link).is_none()
    {
        bail!("not a Vauchi exchange link: {link:?}");
    }

    let mut engine = AppEngine::new(open_vauchi(config)?);
    match their_link {
        None => {
            let screen = engine.navigate_to(AppScreen::LinkExchange);
            let Some(url) = share_url(&screen) else {
                bail!("no share link: create an identity first");
            };
            display::info("Send this link to the other person:");
            println!("{url}");
        }
        Some(link) => {
            let _ = engine.handle_action(UserAction::LinkOpened {
                uri: link.to_string(),
            });
            let consent = engine.current_screen();
            if consent.screen_id == CONSENT_SCREEN {
                if !consented(&consent, yes)? {
                    display::info("Exchange declined.");
                    return Ok(());
                }
                let _ = engine.handle_action(UserAction::ActionPressed {
                    action_id: CONSENT_GRANT.to_string(),
                });
            }
        }
    }

    let deadline = Instant::now() + wait;
    loop {
        let screen = engine.current_screen();
        if SUCCEEDED.contains(&screen.screen_id.as_str()) {
            display::success(&screen.title);
            return Ok(());
        }
        if FAILED.contains(&screen.screen_id.as_str()) {
            bail!("link exchange failed: {}", screen.title);
        }
        let now = Instant::now();
        if now >= deadline {
            bail!("link exchange not completed within {} s", wait.as_secs());
        }
        std::thread::sleep(POLL.min(deadline - now));
        let _ = engine.on_wakeup();
    }
}

/// Core's consent gate for an opened link (ADR-021: never processed
/// without consent).
const CONSENT_SCREEN: &str = "deep_link_consent";
const CONSENT_GRANT: &str = "grant";

/// Shows the consent text Core prepared, then takes `--yes` or asks.
fn consented(screen: &ScreenModel, yes: bool) -> Result<bool> {
    display::info(&screen.title);
    if let Some(subtitle) = &screen.subtitle {
        println!("{subtitle}");
    }
    for component in &screen.components {
        match component {
            Component::Text { content, .. } | Component::Banner { text: content, .. } => {
                println!("{content}");
            }
            _ => {}
        }
    }
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

fn share_url(screen: &ScreenModel) -> Option<String> {
    screen
        .components
        .iter()
        .find_map(|component| match component {
            Component::Text { id, content, .. } if id == "link_url" && !content.is_empty() => {
                Some(content.clone())
            }
            _ => None,
        })
}
