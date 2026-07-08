// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Diagnostic commands for exchange layer inspection.

use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;
use vauchi_core::exchange::transport::animated_qr::{AnimatedQrConfig, AnimatedQrSession};

use crate::display;

/// Diagnostic subcommands.
#[derive(Subcommand)]
pub enum DiagCommands {
    /// Pretty-print a JSON trace file
    Trace {
        /// Path to the JSON trace file
        file: PathBuf,
    },

    /// Animated QR utilities
    #[command(subcommand)]
    AnimatedQr(AnimatedQrCommands),
}

/// Animated QR subcommands.
#[derive(Subcommand)]
pub enum AnimatedQrCommands {
    /// Encode a file as animated QR frames (text output)
    Encode {
        /// Path to the file to encode
        file: PathBuf,

        /// Frames per second (advisory)
        #[arg(long, default_value = "10")]
        fps: u8,

        /// Maximum raw bytes per frame chunk
        #[arg(long, default_value = "400")]
        chunk_size: usize,
    },
}

/// Run the `vauchi diag trace` subcommand.
pub fn trace(file: &PathBuf, locale: &str) -> Result<()> {
    let content = std::fs::read_to_string(file)
        .map_err(|e| anyhow::anyhow!("Failed to read trace file '{}': {}", file.display(), e))?;

    let value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse JSON: {}", e))?;

    let pretty = serde_json::to_string_pretty(&value)?;
    println!("{}", pretty);

    if let Some(events) = value.as_array() {
        println!();
        println!("{}", display::t("cli.cmd.diag.trace.summary", locale));
        println!(
            "{}",
            display::tf(
                "cli.cmd.diag.trace.total_events",
                locale,
                &[("count", &events.len().to_string())]
            )
        );

        if let (Some(first), Some(last)) = (events.first(), events.last())
            && let (Some(t0), Some(t1)) = (
                first.get("timestamp_us").and_then(|v| v.as_u64()),
                last.get("timestamp_us").and_then(|v| v.as_u64()),
            )
        {
            let duration_ms = (t1 - t0) as f64 / 1000.0;
            println!("Duration: {:.2}ms", duration_ms);
        }
    }

    Ok(())
}

/// Run the `vauchi diag animated-qr encode` subcommand.
pub fn animated_qr_encode(file: &PathBuf, fps: u8, chunk_size: usize, locale: &str) -> Result<()> {
    let payload = std::fs::read(file)
        .map_err(|e| anyhow::anyhow!("Failed to read file '{}': {}", file.display(), e))?;

    let config = AnimatedQrConfig {
        fps,
        chunk_size,
        ..Default::default()
    };

    let mut session = AnimatedQrSession::new_sender(payload.clone(), config);
    let frame_count = session.frame_count();

    println!("{}", display::t("cli.cmd.diag.animated_qr.title", locale));
    println!(
        "{}",
        display::tf(
            "cli.cmd.diag.animated_qr.input",
            locale,
            &[("file", &file.display().to_string())]
        )
    );
    println!(
        "{}",
        display::tf(
            "cli.cmd.diag.animated_qr.payload",
            locale,
            &[("size", &payload.len().to_string())]
        )
    );
    println!(
        "{}",
        display::tf(
            "cli.cmd.diag.animated_qr.chunk_size",
            locale,
            &[("size", &chunk_size.to_string())]
        )
    );
    println!(
        "{}",
        display::tf(
            "cli.cmd.diag.animated_qr.fps",
            locale,
            &[("fps", &fps.to_string())]
        )
    );
    println!(
        "{}",
        display::tf(
            "cli.cmd.diag.animated_qr.frames",
            locale,
            &[("count", &frame_count.to_string())]
        )
    );
    println!();

    for i in 0..frame_count {
        if let Some(frame) = session.next_frame() {
            println!(
                "{}",
                display::tf(
                    "cli.cmd.diag.animated_qr.frame",
                    locale,
                    &[
                        ("current", &(i + 1).to_string()),
                        ("total", &frame_count.to_string()),
                    ]
                )
            );
            println!("  {}", frame);
        }
    }

    Ok(())
}
