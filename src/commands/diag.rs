// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Diagnostic commands for transport layer inspection.

use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;
use vauchi_core::exchange::transport::animated_qr::{AnimatedQrConfig, AnimatedQrSession};
use vauchi_core::exchange::transport::channel::TransportType;
use vauchi_core::exchange::transport::diagnostics::TransportDiagnostics;
use vauchi_core::exchange::transport::mock::MockTransportChannel;

/// Diagnostic subcommands.
#[derive(Subcommand)]
pub enum DiagCommands {
    /// Probe available transports and display a status table
    Transport,

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

/// Run the `vauchi diag transport` subcommand.
pub fn transport() -> Result<()> {
    let transports: Vec<Box<dyn vauchi_core::exchange::transport::channel::TransportChannel>> = vec![
        Box::new(MockTransportChannel::new(TransportType::WifiAware).with_available(false)),
        Box::new(MockTransportChannel::new(TransportType::Ble).with_available(false)),
        Box::new(MockTransportChannel::new(TransportType::AnimatedQr)),
        Box::new(MockTransportChannel::new(TransportType::StaticQr)),
        Box::new(MockTransportChannel::new(TransportType::Nfc).with_available(false)),
        Box::new(MockTransportChannel::new(TransportType::Tcp).with_available(false)),
    ];

    let diag = TransportDiagnostics::new(transports);
    let results = diag.probe_all();

    println!("Transport Availability");
    println!("{:<16} {:<12} {}", "Transport", "Available", "Note");
    println!("{:-<16} {:-<12} {:-<30}", "", "", "");

    for result in &results {
        let status = if result.available { "yes" } else { "no" };
        let note = result.error.as_deref().unwrap_or(if result.available {
            "ready (mock)"
        } else {
            "not available on CLI"
        });
        println!("{:<16} {:<12} {}", result.transport, status, note);
    }

    let available_count = results.iter().filter(|r| r.available).count();
    println!();
    println!("{}/{} transports available", available_count, results.len());

    Ok(())
}

/// Run the `vauchi diag trace` subcommand.
pub fn trace(file: &PathBuf) -> Result<()> {
    let content = std::fs::read_to_string(file)
        .map_err(|e| anyhow::anyhow!("Failed to read trace file '{}': {}", file.display(), e))?;

    let value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse JSON: {}", e))?;

    let pretty = serde_json::to_string_pretty(&value)?;
    println!("{}", pretty);

    // Print summary if it looks like a trace event array
    if let Some(events) = value.as_array() {
        println!();
        println!("--- Trace Summary ---");
        println!("Total events: {}", events.len());

        if let (Some(first), Some(last)) = (events.first(), events.last()) {
            if let (Some(t0), Some(t1)) = (
                first.get("timestamp_us").and_then(|v| v.as_u64()),
                last.get("timestamp_us").and_then(|v| v.as_u64()),
            ) {
                let duration_ms = (t1 - t0) as f64 / 1000.0;
                println!("Duration: {:.2}ms", duration_ms);
            }
        }
    }

    Ok(())
}

/// Run the `vauchi diag animated-qr encode` subcommand.
pub fn animated_qr_encode(file: &PathBuf, fps: u8, chunk_size: usize) -> Result<()> {
    let payload = std::fs::read(file)
        .map_err(|e| anyhow::anyhow!("Failed to read file '{}': {}", file.display(), e))?;

    let config = AnimatedQrConfig {
        fps,
        chunk_size,
        ..Default::default()
    };

    let mut session = AnimatedQrSession::new_sender(payload.clone(), config);
    let frame_count = session.frame_count();

    println!("Animated QR Encoding");
    println!("  Input:      {}", file.display());
    println!("  Payload:    {} bytes", payload.len());
    println!("  Chunk size: {} bytes", chunk_size);
    println!("  FPS:        {}", fps);
    println!("  Frames:     {}", frame_count);
    println!();

    for i in 0..frame_count {
        if let Some(frame) = session.next_frame() {
            println!("Frame {}/{}:", i + 1, frame_count);
            println!("  {}", frame);
        }
    }

    Ok(())
}
