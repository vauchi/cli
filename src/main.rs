// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Vauchi CLI
//!
//! Command-line interface for Vauchi - privacy-focused contact card exchange.

mod args;
mod commands;
mod config;
mod dispatch;
mod display;
mod protocol;
mod ui;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use args::Cli;
use config::CliConfig;

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

    dispatch::run(cli.command, &config, cli.pin.as_deref(), &cli.locale).await
}
