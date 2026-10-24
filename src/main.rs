// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Vauchi CLI
//!
//! Command-line interface for Vauchi - privacy-focused contact card exchange.

mod args;
mod clock;
mod commands;
mod config;
mod dispatch;
mod display;
mod profile_access;
mod raw;
mod ui;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use vauchi_app::i18n::init as init_i18n;

use args::Cli;
use config::CliConfig;

/// Try to load runtime locale files so user-visible strings can be translated.
/// Errors are non-fatal: the bundled English fallback is used when no locale
/// directory is found.
fn try_init_i18n() {
    if let Some(dir) = std::env::var_os("VAUCHI_LOCALES_DIR") {
        let _ = init_i18n(Path::new(&dir));
        return;
    }

    // Cargo-built integration tests run from the package root; the workspace
    // locales directory is one level above.
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let workspace_locales = PathBuf::from(manifest_dir)
            .parent()
            .map(|p| p.join("locales"))
            .filter(|p| p.is_dir());
        if let Some(dir) = workspace_locales {
            let _ = init_i18n(&dir);
            return;
        }
    }

    if let Some(dir) = std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(install_locales_dir)
    {
        let _ = init_i18n(&dir);
    }
}

/// The locales folder of the binary's own install layout: next to it,
/// `<prefix>/share/vauchi/locales`, or the workspace's for a `target/<profile>`
/// build. Never an arbitrary ancestor or the working directory, where anyone
/// able to write a `locales` folder would supply the CLI's text
/// (vauchi/private#792).
fn install_locales_dir(exe: &Path) -> Option<PathBuf> {
    let exe_dir = exe.parent()?;
    let mut candidates = vec![
        exe_dir.join("locales"),
        exe_dir.join("../share/vauchi/locales"),
    ];
    if exe_dir.parent().and_then(Path::file_name) == Some("target".as_ref()) {
        candidates.push(exe_dir.join("../../../locales"));
    }
    candidates.into_iter().find(|dir| dir.is_dir())
}

#[tokio::main]
async fn main() -> ExitCode {
    env_logger::init();
    try_init_i18n();

    let cli = Cli::parse();

    let data_dir = match config::resolve_data_dir(cli.data_dir, dirs::data_dir()) {
        Ok(dir) => dir,
        Err(error) => {
            eprintln!("{}: {error}", display::t("error.title", &cli.locale));
            return ExitCode::FAILURE;
        }
    };

    let config = CliConfig {
        data_dir,
        relay_url: cli.relay,
        ohttp_relay_url: cli.ohttp_relay,
        relay_anchor: cli.relay_anchor,
        ohttp_delayed: cli.ohttp_delayed,
        delayed_wait: config::DelayedWait {
            poll: std::time::Duration::from_secs(cli.ohttp_delayed_poll_secs),
            max_wait: std::time::Duration::from_secs(cli.ohttp_delayed_wait_secs),
        },
        raw: cli.raw,
    };

    let pin = config::secret_from_env("VAUCHI_PIN");
    match dispatch::run(cli.command, &config, pin.as_deref(), &cli.locale).await {
        Ok(code) => ExitCode::from(code),
        // Same report the `Result`-returning main printed before exit codes
        // became Core-owned outcomes.
        Err(error) => {
            eprintln!("{}: {error:?}", display::t("error.title", &cli.locale));
            ExitCode::FAILURE
        }
    }
}
