// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared helpers for CLI commands.

use anyhow::{Result, bail};
use std::sync::mpsc;
use vauchi_core::{AuthMode, Vauchi, VauchiConfig, VauchiEvent};

use crate::config::CliConfig;

/// Builds a VauchiConfig from the CLI config (storage path, relay, key).
/// Transport overrides (OHTTP) are layered on by callers that need them.
fn base_wb_config(config: &CliConfig) -> Result<VauchiConfig> {
    Ok(VauchiConfig::with_storage_path(config.storage_path())
        .with_relay_url(&config.relay_url)
        .with_storage_key(config.storage_key()?))
}

/// Opens Vauchi from the config and loads the identity.
///
/// Initialization state is owned by core: a [`Vauchi`] instance auto-loads
/// the identity from its encrypted storage. The legacy `identity.json` file
/// is only a migration source for installs that predate core storage; when
/// core storage is empty and no legacy file exists, Vauchi is not
/// initialized.
pub(crate) fn open_vauchi(config: &CliConfig) -> Result<Vauchi> {
    // `mut` is only needed on debug builds where the direct-HTTP escape
    // hatch below may flip `ohttp.allow_direct`.
    let mut wb_config = base_wb_config(config)?;

    // Explicit OHTTP-relay override (`--ohttp-relay`). When unset, core derives
    // the OHTTP endpoint from the relay URL (production → ohttp.vauchi.app;
    // self-hosted/local → the relay URL itself). See problem
    // 2026-05-25-relay-ohttp-forward-hop-502.
    if let Some(ref ohttp_relay_url) = config.ohttp_relay_url {
        wb_config = wb_config.with_ohttp_relay_url(ohttp_relay_url);
    }

    // Test-only override: `VAUCHI_OVERRIDE_BUNDLED_OHTTP_KEY_HEX` (a
    // hex-encoded RFC 9458 KeyConfig) takes precedence over the
    // compiled-in `BUNDLED_OHTTP_KEY`. Lets the e2e orchestrator
    // inject a freshly-spawned local relay's ephemeral gateway key
    // so the release cli (which compiles out the `VAUCHI_ALLOW_DIRECT`
    // hatch above) can still encap to a key the local relay can
    // decrypt. Release-allowed but WARN-loud — mirrors the F2 pattern
    // in `relay/src/main.rs` (`RELAY_VERSION_CHANGED_AT_SECS`). The
    // override changes which key bytes are used; it does NOT enable
    // direct fetch, so ADR-037 IP-privacy properties hold. Production
    // deployments must NOT set this env var. See problem record
    // `_private/docs/problems/2026-05-04-f13-cli-bundled-key-injection-for-e2e/`.
    if let Ok(hex) = std::env::var("VAUCHI_OVERRIDE_BUNDLED_OHTTP_KEY_HEX") {
        let bytes = hex::decode(hex.trim()).map_err(|e| {
            anyhow::anyhow!("VAUCHI_OVERRIDE_BUNDLED_OHTTP_KEY_HEX is not valid hex: {e}")
        })?;
        // This diagnostic must remain on stderr: contact-list output is
        // machine-parsed by E2E and other callers.
        eprintln!(
            "OHTTP bundled key overridden via \
             VAUCHI_OVERRIDE_BUNDLED_OHTTP_KEY_HEX ({} bytes) — \
             must NOT be set in production",
            bytes.len()
        );
        wb_config.ohttp.bundled_gateway_key = Some(bytes);
    }

    let mut wb = build_vauchi(wb_config)?;

    // Core auto-loads identity from its storage. The legacy identity file
    // is only a fallback for pre-core-storage installs — and a *migration*,
    // not a permanent home: set_identity is in-memory only, and
    // construction-time migrations (e.g. field-centric visibility) only run
    // when the identity loads from storage, so persist it there once.
    if wb.identity().is_none() {
        if !config.is_initialized() {
            bail!("Vauchi not initialized. Run 'vauchi init <name>' first.");
        }
        let identity = config.import_local_identity()?;
        wb.storage()
            .identity()
            .save_identity(&identity.to_storage_bytes(), identity.display_name())
            .map_err(|e| anyhow::anyhow!("Failed to migrate identity into core storage: {e}"))?;
        wb.set_identity(identity)?;
    }

    adopt_legacy_aha_tracker(config, &wb);

    Ok(wb)
}

/// Folds a pre-ADR-069 `aha_tracker.json` into core's encrypted ledger.
///
/// The CLI used to keep its own tracker beside core's. Without this, every
/// milestone an existing user already saw would fire again the first time
/// they run a build where core owns the decision. Union, never overwrite:
/// each side may hold moments the other never saw.
fn adopt_legacy_aha_tracker(config: &CliConfig, wb: &Vauchi) {
    let path = config.data_dir.join("aha_tracker.json");
    let Ok(json) = std::fs::read_to_string(&path) else {
        return;
    };
    let Ok(legacy) = vauchi_core::AhaMomentTracker::from_json(&json) else {
        // Leave an unreadable file in place rather than silently dropping the
        // only record of what the user has already seen.
        return;
    };
    let seen: Vec<_> = vauchi_core::AhaMomentType::all()
        .iter()
        .filter(|moment| legacy.has_seen(**moment))
        .copied()
        .collect();
    if wb.mark_aha_moments_seen(&seen).is_ok() {
        let _ = std::fs::remove_file(&path);
    }
}

/// Returns true when an identity exists in core storage or in the legacy
/// identity file. Replaces direct `CliConfig::is_initialized` checks so
/// callers follow core-owned initialization state.
///
/// Fails CLOSED: probe errors (keychain failure, undecryptable storage)
/// propagate, so a destructive caller aborts instead of skipping its
/// confirmation prompt. The probe uses the base config only — the
/// OHTTP-override warning belongs to real opens, not predicates.
pub(crate) fn identity_exists(config: &CliConfig) -> Result<bool> {
    let wb = build_vauchi(base_wb_config(config)?)?;
    if wb.identity().is_some() {
        return Ok(true);
    }
    Ok(config.is_initialized())
}

/// Removes all local identity state: the core database (including WAL
/// sidecars — an orphan `vauchi.db-wal` can replay pre-reset rows into a
/// freshly created DB) and the legacy identity file. Callers must validate
/// whatever replaces this state BEFORE resetting.
pub(crate) fn reset_local_state(config: &CliConfig) -> Result<()> {
    let storage_path = config.storage_path();
    let paths = [
        storage_path.clone(),
        storage_path.with_extension("db-wal"),
        storage_path.with_extension("db-shm"),
        config.identity_path(),
    ];
    for path in paths {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// Builds the Vauchi instance for `wb_config`. Only the dedicated
/// `e2e-test-clock` binary can pin `VAUCHI_TEST_CLOCK_EPOCH` and thread
/// its clock into core. Shipping binaries always retain the system clock.
fn build_vauchi(wb_config: VauchiConfig) -> Result<Vauchi> {
    let clock = crate::clock::is_pinned().then(crate::clock::shared);
    build_vauchi_with_clock(wb_config, clock)
}

fn build_vauchi_with_clock(
    wb_config: VauchiConfig,
    clock: Option<std::sync::Arc<dyn vauchi_core::clock::Clock>>,
) -> Result<Vauchi> {
    match clock {
        Some(clock) => Ok(Vauchi::new_with(
            wb_config,
            clock,
            vauchi_core::rng::OsSecureRng::shared(),
            None,
        )?),
        None => Ok(Vauchi::new(wb_config)?),
    }
}

/// Opens Vauchi and authenticates if a PIN is provided.
///
/// When `pin` is `Some`, calls [`Vauchi::authenticate`] which sets the auth
/// mode (Normal or Duress). In duress mode, core automatically queues silent
/// alerts and `list_contacts()` returns decoy contacts.
///
/// When `pin` is `None` and no app password is configured, the instance
/// remains unauthenticated (backward-compatible).
///
/// When `pin` is `None` but an app password IS configured, returns an error
/// requiring authentication.
pub(crate) fn open_vauchi_authenticated(config: &CliConfig, pin: Option<&str>) -> Result<Vauchi> {
    let mut wb = open_vauchi(config)?;

    match pin {
        Some(p) => {
            wb.authenticate(p)?;
            Ok(wb)
        }
        None => {
            if wb.is_password_enabled()? {
                bail!(
                    "App password is configured. Use --pin to authenticate.\n\
                     Hint: vauchi --pin <PIN> contacts list"
                );
            }
            Ok(wb)
        }
    }
}

/// Returns the auth mode string for display.
pub(crate) fn auth_mode_label(mode: AuthMode) -> &'static str {
    match mode {
        AuthMode::Normal => "normal",
        AuthMode::Duress => "duress",
        AuthMode::Unauthenticated => "unauthenticated",
        _ => "unknown",
    }
}

/// Registers an event handler that captures events for the activity log (ADR-031).
///
/// Use with `drain_activity_log` to persist events at the end of a command.
pub(crate) fn register_activity_log_handler(wb: &Vauchi) -> mpsc::Receiver<VauchiEvent> {
    let (event_tx, event_rx) = mpsc::channel();
    let event_tx = std::sync::Mutex::new(event_tx);
    wb.add_event_handler(std::sync::Arc::new(move |event| {
        if let Ok(tx) = event_tx.lock() {
            let _ = tx.send(event);
        }
    }));
    event_rx
}

/// Drains captured events and persists them to the activity log.
///
/// Usually called at the end of a command's successful execution.
pub(crate) fn drain_activity_log(wb: &Vauchi, rx: mpsc::Receiver<VauchiEvent>) {
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    // Core decides when a milestone fires and hands over the resolved moment;
    // rendering it is all that is left to a shell (ADR-069).
    for event in &events {
        if let VauchiEvent::AhaMomentTriggered { moment } = event {
            crate::display::display_aha_moment(moment);
        }
    }
    if !events.is_empty() {
        // Persisted activity-log timestamp — routes through the injectable
        // CLI clock so E2E clock-skew scenarios control `created_at`.
        let now = crate::clock::unix_seconds();
        let _ =
            vauchi_app::activity_log_writer::ActivityLogWriter::write(wb.storage(), &events, now);
    }
}

// INLINE_TEST_REQUIRED: Binary crate without lib.rs - tests cannot be external
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "e2e-test-clock")]
    use std::time::SystemTime;
    use tempfile::tempdir;
    use vauchi_core::Identity;
    #[cfg(feature = "e2e-test-clock")]
    use vauchi_core::clock::Clock;

    #[cfg(feature = "e2e-test-clock")]
    struct FixedClock(SystemTime);

    #[cfg(feature = "e2e-test-clock")]
    impl Clock for FixedClock {
        fn now(&self) -> SystemTime {
            self.0
        }
    }

    // @internal
    #[cfg(feature = "e2e-test-clock")]
    #[test]
    fn pinned_clock_threads_into_core_storage_clock() {
        let temp_dir = tempdir().unwrap();
        let wb_config = VauchiConfig::with_storage_path(temp_dir.path().join("clock-pinned.db"))
            .with_storage_key(vauchi_core::crypto::SymmetricKey::generate());
        let pinned = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        let wb = build_vauchi_with_clock(wb_config, Some(std::sync::Arc::new(FixedClock(pinned))))
            .expect("vauchi should open with pinned clock");

        assert_eq!(
            wb.storage().clock().unix_seconds(),
            1_700_000_000,
            "core storage clock must follow VAUCHI_TEST_CLOCK_EPOCH"
        );
    }

    // @internal
    #[test]
    fn unpinned_clock_keeps_system_storage_clock() {
        let temp_dir = tempdir().unwrap();
        let wb_config = VauchiConfig::with_storage_path(temp_dir.path().join("clock-system.db"))
            .with_storage_key(vauchi_core::crypto::SymmetricKey::generate());

        let before = std::time::SystemTime::now();
        let wb =
            build_vauchi_with_clock(wb_config, None).expect("vauchi should open with system clock");
        let after = std::time::SystemTime::now();
        let got = wb
            .storage()
            .clock()
            .now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let lo = before
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let hi = after
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        assert!(
            (lo..=hi + 1).contains(&got),
            "unset VAUCHI_TEST_CLOCK_EPOCH must keep the system clock: got {got} outside [{lo}, {hi}]"
        );
    }

    /// Helper: create a CliConfig with a temp dir and initialize identity.
    fn setup_initialized_config() -> (tempfile::TempDir, CliConfig) {
        let temp_dir = tempdir().unwrap();
        let config = CliConfig {
            data_dir: temp_dir.path().to_path_buf(),
            relay_url: "ws://localhost:8080".to_string(),
            ohttp_relay_url: None,
            raw: false,
        };
        let identity = Identity::create("Test User", crate::clock::shared().unix_seconds());
        config.save_local_identity(&identity).unwrap();
        (temp_dir, config)
    }

    /// Helper: set up app password and optional duress PIN on a Vauchi instance.
    fn setup_passwords(config: &CliConfig, password: &str, duress_pin: Option<&str>) {
        let mut wb = open_vauchi(config).unwrap();
        wb.setup_app_password(password).unwrap();
        if let Some(pin) = duress_pin {
            wb.setup_duress_password(pin).unwrap();
        }
    }

    #[test]
    fn test_open_vauchi_uninitialized_returns_error() {
        let temp_dir = tempdir().unwrap();
        let config = CliConfig {
            data_dir: temp_dir.path().to_path_buf(),
            relay_url: "ws://localhost:8080".to_string(),
            ohttp_relay_url: None,
            raw: false,
        };

        let result = open_vauchi(&config);

        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("expected error for uninitialized vauchi"),
        };
        assert_eq!(
            err.to_string(),
            "Vauchi not initialized. Run 'vauchi init <name>' first."
        );
    }

    #[test]
    fn test_open_vauchi_initialized_returns_vauchi_with_identity() {
        let temp_dir = tempdir().unwrap();
        let config = CliConfig {
            data_dir: temp_dir.path().to_path_buf(),
            relay_url: "ws://localhost:8080".to_string(),
            ohttp_relay_url: None,
            raw: false,
        };

        let identity = Identity::create("Test User", crate::clock::shared().unix_seconds());
        config
            .save_local_identity(&identity)
            .expect("save identity");

        let wb = open_vauchi(&config).expect("open_vauchi should succeed");

        let loaded_identity = wb.identity().expect("identity should be loaded");
        assert_eq!(loaded_identity.display_name(), "Test User");
    }

    /// The legacy identity.json fallback is a *migration*: after one open,
    /// the identity lives in core storage and the file can go away.
    // @internal
    #[test]
    fn test_open_vauchi_migrates_legacy_identity_file_into_core_storage() {
        let temp_dir = tempdir().unwrap();
        let config = CliConfig {
            data_dir: temp_dir.path().to_path_buf(),
            relay_url: "ws://localhost:8080".to_string(),
            ohttp_relay_url: None,
            raw: false,
        };

        let identity = Identity::create("Test User", crate::clock::shared().unix_seconds());
        config
            .save_local_identity(&identity)
            .expect("save identity");

        let wb = open_vauchi(&config).expect("first open loads the legacy file");
        drop(wb);

        std::fs::remove_file(config.identity_path()).expect("remove legacy file");

        let wb2 = open_vauchi(&config)
            .expect("second open must succeed from core storage without identity.json");
        assert_eq!(
            wb2.identity()
                .expect("identity from core storage")
                .display_name(),
            "Test User"
        );
    }

    #[test]
    fn test_open_vauchi_uses_configured_storage_path() {
        let temp_dir = tempdir().unwrap();
        let config = CliConfig {
            data_dir: temp_dir.path().to_path_buf(),
            relay_url: "ws://localhost:9999".to_string(),
            ohttp_relay_url: None,
            raw: false,
        };

        let identity = Identity::create("Storage Path Test", crate::clock::shared().unix_seconds());
        config
            .save_local_identity(&identity)
            .expect("save identity");

        let result = open_vauchi(&config);
        assert!(
            result.is_ok(),
            "open_vauchi should succeed with valid config: {:?}",
            result.err()
        );
    }

    // ================================================================
    // open_vauchi_authenticated tests
    // Feature: duress_pin.feature @unlock
    // ================================================================

    /// Feature: duress_pin.feature @unlock @implemented
    /// No password configured + no PIN → succeeds unauthenticated.
    #[test]
    fn test_authenticated_no_password_no_pin_succeeds() {
        let (_dir, config) = setup_initialized_config();

        let wb = open_vauchi_authenticated(&config, None).expect("should succeed without password");
        assert_eq!(wb.auth_mode(), AuthMode::Unauthenticated);
    }

    /// Feature: duress_pin.feature @unlock @implemented
    /// Password configured + no PIN → error requiring authentication.
    #[test]
    fn test_authenticated_password_set_no_pin_errors() {
        let (_dir, config) = setup_initialized_config();
        setup_passwords(&config, "correct-password", None);

        let result = open_vauchi_authenticated(&config, None);
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("should require PIN when password is set"),
        };
        assert!(
            err.to_string().contains("--pin"),
            "Error should mention --pin flag, got: {}",
            err
        );
    }

    /// Feature: duress_pin.feature @unlock @implemented
    /// Normal PIN shows real mode.
    #[test]
    fn test_authenticated_normal_pin_returns_normal_mode() {
        let (_dir, config) = setup_initialized_config();
        setup_passwords(&config, "correct-password", Some("duress-pin"));

        let wb = open_vauchi_authenticated(&config, Some("correct-password"))
            .expect("normal PIN should authenticate");
        assert_eq!(wb.auth_mode(), AuthMode::Normal);
    }

    /// Feature: duress_pin.feature @unlock @implemented
    /// Duress PIN shows duress mode.
    #[test]
    fn test_authenticated_duress_pin_returns_duress_mode() {
        let (_dir, config) = setup_initialized_config();
        setup_passwords(&config, "correct-password", Some("duress-pin"));

        let wb = open_vauchi_authenticated(&config, Some("duress-pin"))
            .expect("duress PIN should authenticate");
        assert_eq!(wb.auth_mode(), AuthMode::Duress);
    }

    /// Feature: duress_pin.feature @unlock @implemented
    /// Invalid PIN returns error.
    #[test]
    fn test_authenticated_invalid_pin_returns_error() {
        let (_dir, config) = setup_initialized_config();
        setup_passwords(&config, "correct-password", None);

        let result = open_vauchi_authenticated(&config, Some("wrong-pin"));
        let err = match result {
            Err(e) => e,
            Ok(_) => panic!("invalid PIN should fail"),
        };
        assert!(
            err.to_string().contains("invalid"),
            "Error should mention invalid, got: {}",
            err
        );
    }

    /// auth_mode_label returns correct strings.
    #[test]
    fn test_auth_mode_label_values() {
        assert_eq!(auth_mode_label(AuthMode::Normal), "normal");
        assert_eq!(auth_mode_label(AuthMode::Duress), "duress");
        assert_eq!(
            auth_mode_label(AuthMode::Unauthenticated),
            "unauthenticated"
        );
    }
}
