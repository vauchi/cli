// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Where an installed binary may load its locale files from: only its own
//! install layout, never an arbitrary ancestor folder or the working
//! directory (GitLab SAST 409779120, vauchi/private#792).

use std::path::{Path, PathBuf};
use std::process::Command;

const PLANTED: &str = "PLANTED";

/// The real German locale with `error.title` replaced, so a run shows
/// which file it loaded.
fn planted_german() -> String {
    let real = Path::new(env!("CARGO_MANIFEST_DIR")).join("../locales/de.json");
    let mut locale: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&real).expect("read de.json"))
            .expect("parse de.json");
    locale["error.title"] = serde_json::Value::String(PLANTED.into());
    locale.to_string()
}

fn plant(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("de.json"), planted_german()).unwrap();
}

fn install(exe_dir: &Path) -> PathBuf {
    std::fs::create_dir_all(exe_dir).unwrap();
    let exe = exe_dir.join("vauchi");
    std::fs::copy(env!("CARGO_BIN_EXE_vauchi"), &exe).expect("copy the binary");
    exe
}

/// A German error line from `exe`, run in `cwd` with no locale override.
fn error_line(exe: &Path, cwd: &Path) -> String {
    let output = Command::new(exe)
        .current_dir(cwd)
        .env_remove("VAUCHI_LOCALES_DIR")
        .env_remove("CARGO_MANIFEST_DIR")
        .arg("--data-dir")
        .arg(cwd.join("data"))
        .args(["--locale", "de", "card", "add", "email"])
        .output()
        .expect("run vauchi");
    assert!(
        !output.status.success(),
        "card add without arguments must fail"
    );
    String::from_utf8_lossy(&output.stderr).to_string()
}

// @internal
#[test]
fn a_locales_folder_above_the_install_is_never_loaded() {
    let root = tempfile::tempdir().unwrap();
    plant(&root.path().join("locales"));
    let exe = install(&root.path().join("opt/vauchi/bin"));

    let stderr = error_line(&exe, root.path());

    assert!(!stderr.contains(PLANTED), "{stderr}");
}

// @internal
#[test]
fn a_locales_folder_in_the_working_directory_is_never_loaded() {
    let root = tempfile::tempdir().unwrap();
    let exe = install(&root.path().join("bin"));
    let cwd = root.path().join("somewhere");
    plant(&cwd.join("locales"));

    let stderr = error_line(&exe, &cwd);

    assert!(!stderr.contains(PLANTED), "{stderr}");
}

// @internal
#[test]
fn locales_next_to_the_binary_are_loaded() {
    let root = tempfile::tempdir().unwrap();
    let exe = install(&root.path().join("bin"));
    plant(&root.path().join("bin/locales"));

    let stderr = error_line(&exe, root.path());

    assert!(stderr.contains(&format!("{PLANTED}:")), "{stderr}");
}

// @internal
#[test]
fn locales_in_the_install_prefix_share_folder_are_loaded() {
    let root = tempfile::tempdir().unwrap();
    let exe = install(&root.path().join("usr/local/bin"));
    plant(&root.path().join("usr/local/share/vauchi/locales"));

    let stderr = error_line(&exe, root.path());

    assert!(stderr.contains(&format!("{PLANTED}:")), "{stderr}");
}

// @internal
#[test]
fn a_workspace_build_loads_the_workspace_locales() {
    let root = tempfile::tempdir().unwrap();
    let exe = install(&root.path().join("ws/cli/target/release"));
    plant(&root.path().join("ws/locales"));

    let stderr = error_line(&exe, root.path());

    assert!(stderr.contains(&format!("{PLANTED}:")), "{stderr}");
}
