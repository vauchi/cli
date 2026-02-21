// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared helpers for CLI commands.

use anyhow::{bail, Result};
use vauchi_core::network::MockTransport;
use vauchi_core::{Vauchi, VauchiConfig};

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

// INLINE_TEST_REQUIRED: Binary crate without lib.rs - tests cannot be external
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use vauchi_core::Identity;

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
}
