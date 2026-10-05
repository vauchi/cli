// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Device listing, info, join and decommission, and the diag tools,
//! through the binary (vauchi/private#505).

mod common;

use common::Cli;

fn initialized() -> Cli {
    let cli = Cli::new();
    cli.stdout(&["init", "Alice"]);
    cli
}

// @internal
#[test]
fn device_info_shows_when_the_device_was_created() {
    let cli = initialized();

    let info = cli.stdout(&["device", "info"]);
    let created = info
        .lines()
        .find_map(|line| line.trim().strip_prefix("Created:"))
        .unwrap_or_else(|| panic!("{info}"))
        .trim();
    let seconds: u64 = created
        .strip_suffix(" seconds since epoch")
        .unwrap_or_else(|| panic!("{created}"))
        .parse()
        .unwrap();
    assert!(seconds > 1_700_000_000, "{created}");
}

// @internal
#[test]
fn join_with_yes_skips_the_replace_prompt_on_an_initialized_device() {
    let cli = initialized();

    let output = cli.run(&["device", "join", "not-a-link-qr", "--yes"]);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(!stderr.contains("not a terminal"), "{stderr}");
}

// @internal
#[test]
fn decommission_with_yes_wipes_without_asking() {
    let cli = initialized();

    let output = cli.stdout(&["device", "decommission", "--yes"]);

    assert!(
        output.contains("Device decommissioned. 0 contact session(s) wiped."),
        "{output}"
    );
}

// @internal
#[test]
fn a_trace_reports_its_event_count_and_duration() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("trace.json");
    std::fs::write(
        &file,
        r#"[{"timestamp_us": 1000}, {"timestamp_us": 2000}, {"timestamp_us": 3500}]"#,
    )
    .unwrap();

    let output = Cli::new().stdout(&["diag", "trace", file.to_str().unwrap()]);

    assert!(output.contains("3"), "{output}");
    assert!(output.contains("Duration: 2.50ms"), "{output}");
}

// @internal
#[test]
fn animated_qr_frames_follow_the_chunk_size() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("payload.bin");
    std::fs::write(&file, vec![7u8; 1000]).unwrap();
    let frames = |chunk: &str| -> usize {
        let output = Cli::new().stdout(&[
            "diag",
            "animated-qr",
            "encode",
            file.to_str().unwrap(),
            "--chunk-size",
            chunk,
        ]);
        output
            .lines()
            .find_map(|line| line.strip_prefix("Frames:"))
            .unwrap_or_else(|| panic!("{output}"))
            .trim()
            .parse()
            .unwrap()
    };

    assert!(frames("100") > frames("400"));
}

// @internal
#[test]
fn an_unreachable_relay_fails_the_probe() {
    let output = Cli::new().run(&["diag", "ohttp-probe"]);

    assert_eq!(output.status.code(), Some(1));
}
