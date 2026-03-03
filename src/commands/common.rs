// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared helpers for CLI commands.

use anyhow::{bail, Result};
use vauchi_core::network::MockTransport;
use vauchi_core::{AuthMode, Vauchi, VauchiConfig};

use crate::config::CliConfig;

/// Opens Vauchi from the config and loads the identity.
///
/// Checks that Vauchi has been initialized (identity file exists),
/// builds a [`VauchiConfig`] from the CLI config, creates a [`Vauchi`]
/// instance, and loads the local identity into it.
pub(crate) fn open_vauchi(config: &CliConfig) -> Result<Vauchi<MockTransport>> {
    if !config.is_initialized() {
        bail!("Vauchi not initialized. Run 'vauchi init <name>' first.");
    }

    let wb_config = VauchiConfig::with_storage_path(config.storage_path())
        .with_relay_url(&config.relay_url)
        .with_storage_key(config.storage_key()?);

    let mut wb = Vauchi::new(wb_config)?;

    let identity = config.import_local_identity()?;
    wb.set_identity(identity)?;

    Ok(wb)
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
pub(crate) fn open_vauchi_authenticated(
    config: &CliConfig,
    pin: Option<&str>,
) -> Result<Vauchi<MockTransport>> {
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
    }
}

// INLINE_TEST_REQUIRED: Binary crate without lib.rs - tests cannot be external
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use vauchi_core::Identity;

    /// Helper: create a CliConfig with a temp dir and initialize identity.
    fn setup_initialized_config() -> (tempfile::TempDir, CliConfig) {
        let temp_dir = tempdir().unwrap();
        let config = CliConfig {
            data_dir: temp_dir.path().to_path_buf(),
            relay_url: "ws://localhost:8080".to_string(),
        };
        let identity = Identity::create("Test User");
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
        };

        // Initialize: create an identity and save it
        let identity = Identity::create("Test User");
        config
            .save_local_identity(&identity)
            .expect("save identity");

        let wb = open_vauchi(&config).expect("open_vauchi should succeed");

        // Verify the identity was loaded by checking the display name
        let loaded_identity = wb.identity().expect("identity should be loaded");
        assert_eq!(loaded_identity.display_name(), "Test User");
    }

    #[test]
    fn test_open_vauchi_uses_configured_storage_path() {
        let temp_dir = tempdir().unwrap();
        let config = CliConfig {
            data_dir: temp_dir.path().to_path_buf(),
            relay_url: "ws://localhost:9999".to_string(),
        };

        let identity = Identity::create("Storage Path Test");
        config
            .save_local_identity(&identity)
            .expect("save identity");

        // The function should not panic or error — it creates storage at the configured path
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
