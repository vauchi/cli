// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! CLI Configuration

use std::path::PathBuf;

use anyhow::Result;
use vauchi_core::{Identity, IdentityBackup, SymmetricKey};

#[cfg(feature = "secure-storage")]
use vauchi_core::storage::secure::{PlatformKeyring, SecureStorage};

#[cfg(not(feature = "secure-storage"))]
use vauchi_core::storage::secure::{FileKeyStorage, SecureStorage};

/// Legacy hardcoded password used before per-installation backup passwords.
const LEGACY_BACKUP_PASSWORD: &str = "vauchi-local-storage";

/// CLI configuration.
#[derive(Debug, Clone)]
pub struct CliConfig {
    /// Data directory for storage.
    pub data_dir: PathBuf,
    /// Relay server URL.
    pub relay_url: String,
}

/// Key name used for SecureStorage.
const KEY_NAME: &str = "storage_key";

/// Loads or generates a per-installation random fallback key from `data_dir/.fallback-key`.
///
/// Used only when the `secure-storage` feature is disabled. Each installation
/// gets a unique random key instead of a hardcoded constant.
#[cfg(not(feature = "secure-storage"))]
pub(crate) fn load_or_generate_fallback_key(data_dir: &std::path::Path) -> Result<SymmetricKey> {
    use anyhow::Context;

    let key_path = data_dir.join(".fallback-key");

    if key_path.exists() {
        let bytes = std::fs::read(&key_path).context("Failed to read fallback key")?;
        if bytes.len() != 32 {
            anyhow::bail!(
                "Invalid fallback key length ({}), expected 32. Delete {} to regenerate.",
                bytes.len(),
                key_path.display()
            );
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        return Ok(SymmetricKey::from_bytes(arr));
    }

    // Generate a new random key
    let key = SymmetricKey::generate();

    // Ensure parent directory exists
    std::fs::create_dir_all(data_dir).context("Failed to create data directory")?;

    std::fs::write(&key_path, key.as_bytes()).context("Failed to write fallback key")?;

    // Set restrictive permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
            .context("Failed to set fallback key permissions")?;
    }

    Ok(key)
}

/// Loads or generates a per-installation random backup password from `data_dir/.backup-password`.
///
/// Each installation gets a unique random password (32 random bytes, hex-encoded)
/// instead of the old hardcoded `"vauchi-local-storage"` constant.
fn load_or_generate_backup_password(data_dir: &std::path::Path) -> Result<String> {
    use anyhow::Context;

    let password_path = data_dir.join(".backup-password");

    if password_path.exists() {
        let content =
            std::fs::read_to_string(&password_path).context("Failed to read backup password")?;
        let trimmed = content.trim().to_string();
        if trimmed.len() != 64 {
            anyhow::bail!(
                "Invalid backup password length ({}), expected 64 hex chars. Delete {} to regenerate.",
                trimmed.len(),
                password_path.display()
            );
        }
        return Ok(trimmed);
    }

    // Generate a new random password (32 random bytes, hex-encoded = 64 chars)
    let key = SymmetricKey::generate();
    let password: String = key
        .as_bytes()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();

    // Ensure parent directory exists
    std::fs::create_dir_all(data_dir).context("Failed to create data directory")?;

    std::fs::write(&password_path, &password).context("Failed to write backup password")?;

    // Set restrictive permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&password_path, std::fs::Permissions::from_mode(0o600))
            .context("Failed to set backup password permissions")?;
    }

    Ok(password)
}

impl CliConfig {
    /// Returns the storage path for Vauchi data.
    pub fn storage_path(&self) -> PathBuf {
        self.data_dir.join("data.db")
    }

    /// Returns the identity file path.
    pub fn identity_path(&self) -> PathBuf {
        self.data_dir.join("identity.json")
    }

    /// Returns true if the identity file exists.
    pub fn is_initialized(&self) -> bool {
        self.identity_path().exists()
    }

    /// Returns the per-installation backup password for identity persistence.
    pub fn backup_password(&self) -> Result<String> {
        load_or_generate_backup_password(&self.data_dir)
    }

    /// Imports the local identity with migration from legacy hardcoded password.
    ///
    /// Tries the per-installation password first. If that fails, falls back to the
    /// legacy `"vauchi-local-storage"` password and re-exports with the new password.
    pub fn import_local_identity(&self) -> Result<Identity> {
        let password = self.backup_password()?;
        let backup_data = std::fs::read(self.identity_path())?;
        let backup = IdentityBackup::new(backup_data);

        match Identity::import_backup(&backup, &password) {
            Ok(identity) => Ok(identity),
            Err(_) => {
                // Try legacy hardcoded password for migration
                let identity = Identity::import_backup(&backup, LEGACY_BACKUP_PASSWORD)
                    .map_err(|e| anyhow::anyhow!("Failed to import identity: {:?}", e))?;
                // Re-export with per-installation password
                let new_backup = identity
                    .export_backup(&password)
                    .map_err(|e| anyhow::anyhow!("Failed to re-export identity: {:?}", e))?;
                std::fs::write(self.identity_path(), new_backup.as_bytes())?;
                Ok(identity)
            }
        }
    }

    /// Saves the identity to the local persistence file.
    pub fn save_local_identity(&self, identity: &Identity) -> Result<()> {
        let password = self.backup_password()?;
        let backup = identity
            .export_backup(&password)
            .map_err(|e| anyhow::anyhow!("Failed to export backup: {:?}", e))?;
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::write(self.identity_path(), backup.as_bytes())?;
        Ok(())
    }

    /// Loads or creates the storage encryption key using SecureStorage.
    ///
    /// When the `secure-storage` feature is enabled, uses the OS keychain.
    /// Otherwise, falls back to encrypted file storage.
    #[allow(unused_variables)]
    pub fn storage_key(&self) -> Result<SymmetricKey> {
        #[cfg(feature = "secure-storage")]
        {
            let storage = PlatformKeyring::new("vauchi-cli");
            match storage.load_key(KEY_NAME) {
                Ok(Some(bytes)) if bytes.len() == 32 => {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    Ok(SymmetricKey::from_bytes(arr))
                }
                Ok(Some(_)) => {
                    anyhow::bail!("Invalid storage key length in keychain");
                }
                Ok(None) => {
                    let key = SymmetricKey::generate();
                    storage
                        .save_key(KEY_NAME, key.as_bytes())
                        .map_err(|e| anyhow::anyhow!("Failed to save key to keychain: {}", e))?;
                    Ok(key)
                }
                Err(e) => {
                    anyhow::bail!("Keychain error: {}", e);
                }
            }
        }

        #[cfg(not(feature = "secure-storage"))]
        {
            let fallback_key = load_or_generate_fallback_key(&self.data_dir)?;

            let key_dir = self.data_dir.join("keys");
            let storage = FileKeyStorage::new(key_dir, fallback_key);

            match storage.load_key(KEY_NAME) {
                Ok(Some(bytes)) if bytes.len() == 32 => {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    Ok(SymmetricKey::from_bytes(arr))
                }
                Ok(Some(_)) => {
                    anyhow::bail!("Invalid storage key length");
                }
                Ok(None) => {
                    let key = SymmetricKey::generate();
                    storage
                        .save_key(KEY_NAME, key.as_bytes())
                        .map_err(|e| anyhow::anyhow!("Failed to save storage key: {}", e))?;
                    Ok(key)
                }
                Err(e) => {
                    anyhow::bail!("Storage error: {}", e);
                }
            }
        }
    }
}

// INLINE_TEST_REQUIRED: Binary crate without lib.rs - tests cannot be external
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_storage_key_creates_key_on_first_call() {
        let temp_dir = tempdir().unwrap();
        let config = CliConfig {
            data_dir: temp_dir.path().to_path_buf(),
            relay_url: "ws://localhost:8080".to_string(),
        };

        // First call should create a key
        let key = config.storage_key().expect("should create key");

        // Key should be 32 bytes
        assert_eq!(key.as_bytes().len(), 32);
    }

    // Note: When secure-storage feature is enabled, these tests use the OS keychain
    // which is shared across all tests and may not support the same persistence
    // semantics in test environments. These tests verify FileKeyStorage persistence.
    #[cfg(not(feature = "secure-storage"))]
    #[test]
    fn test_storage_key_persists_across_calls() {
        let temp_dir = tempdir().unwrap();
        let config = CliConfig {
            data_dir: temp_dir.path().to_path_buf(),
            relay_url: "ws://localhost:8080".to_string(),
        };

        // First call creates key
        let key1 = config.storage_key().expect("should create key");

        // Second call should return the same key
        let key2 = config.storage_key().expect("should load key");

        assert_eq!(key1.as_bytes(), key2.as_bytes());
    }

    // === Backup Password Tests ===
    // Trace: codebase-review-tracker item #27

    #[test]
    fn test_backup_password_is_not_hardcoded() {
        let temp_dir = tempdir().unwrap();
        let config = CliConfig {
            data_dir: temp_dir.path().to_path_buf(),
            relay_url: "ws://localhost:8080".to_string(),
        };

        let password = config.backup_password().expect("should generate password");
        assert_ne!(password, "vauchi-local-storage");
        assert_eq!(password.len(), 64); // 32 bytes hex-encoded
    }

    #[test]
    fn test_backup_password_persists_across_calls() {
        let temp_dir = tempdir().unwrap();
        let config = CliConfig {
            data_dir: temp_dir.path().to_path_buf(),
            relay_url: "ws://localhost:8080".to_string(),
        };

        let pw1 = config.backup_password().unwrap();
        let pw2 = config.backup_password().unwrap();
        assert_eq!(pw1, pw2);
    }

    #[test]
    fn test_backup_password_differs_per_install() {
        let temp1 = tempdir().unwrap();
        let temp2 = tempdir().unwrap();
        let config1 = CliConfig {
            data_dir: temp1.path().to_path_buf(),
            relay_url: "ws://localhost:8080".to_string(),
        };
        let config2 = CliConfig {
            data_dir: temp2.path().to_path_buf(),
            relay_url: "ws://localhost:8080".to_string(),
        };

        let pw1 = config1.backup_password().unwrap();
        let pw2 = config2.backup_password().unwrap();
        assert_ne!(pw1, pw2);
    }

    #[test]
    fn test_migration_from_legacy_password() {
        let temp_dir = tempdir().unwrap();
        let config = CliConfig {
            data_dir: temp_dir.path().to_path_buf(),
            relay_url: "ws://localhost:8080".to_string(),
        };

        // Create an identity encrypted with the old hardcoded password
        let identity = Identity::create("Test User");
        let backup = identity.export_backup("vauchi-local-storage").unwrap();
        std::fs::create_dir_all(&config.data_dir).unwrap();
        std::fs::write(config.identity_path(), backup.as_bytes()).unwrap();

        // Import should succeed via migration
        let imported = config.import_local_identity().unwrap();
        assert_eq!(imported.display_name(), "Test User");

        // After migration, the file should be re-encrypted with new password
        let new_password = config.backup_password().unwrap();
        let new_backup_data = std::fs::read(config.identity_path()).unwrap();
        let new_backup = IdentityBackup::new(new_backup_data);
        let reimported = Identity::import_backup(&new_backup, &new_password).unwrap();
        assert_eq!(reimported.display_name(), "Test User");
    }

    #[test]
    fn test_new_install_uses_generated_password() {
        let temp_dir = tempdir().unwrap();
        let config = CliConfig {
            data_dir: temp_dir.path().to_path_buf(),
            relay_url: "ws://localhost:8080".to_string(),
        };

        // Create identity and save with new password
        let identity = Identity::create("Test User");
        config.save_local_identity(&identity).unwrap();

        // Import should succeed with generated password
        let imported = config.import_local_identity().unwrap();
        assert_eq!(imported.display_name(), "Test User");
    }

    #[test]
    fn test_re_export_after_migration_uses_new_password() {
        let temp_dir = tempdir().unwrap();
        let config = CliConfig {
            data_dir: temp_dir.path().to_path_buf(),
            relay_url: "ws://localhost:8080".to_string(),
        };

        // Create identity with legacy password
        let identity = Identity::create("Migration User");
        let backup = identity.export_backup("vauchi-local-storage").unwrap();
        std::fs::create_dir_all(&config.data_dir).unwrap();
        std::fs::write(config.identity_path(), backup.as_bytes()).unwrap();

        // Import triggers migration
        config.import_local_identity().unwrap();

        // Old password should no longer work
        let new_data = std::fs::read(config.identity_path()).unwrap();
        let new_backup = IdentityBackup::new(new_data);
        assert!(Identity::import_backup(&new_backup, "vauchi-local-storage").is_err());
    }

    #[cfg(not(feature = "secure-storage"))]
    #[test]
    fn test_storage_key_persists_across_config_instances() {
        let temp_dir = tempdir().unwrap();
        let data_dir = temp_dir.path().to_path_buf();

        // First config instance creates key
        let config1 = CliConfig {
            data_dir: data_dir.clone(),
            relay_url: "ws://localhost:8080".to_string(),
        };
        let key1 = config1.storage_key().expect("should create key");

        // Second config instance with same data_dir loads same key
        let config2 = CliConfig {
            data_dir,
            relay_url: "ws://localhost:8080".to_string(),
        };
        let key2 = config2.storage_key().expect("should load key");

        assert_eq!(key1.as_bytes(), key2.as_bytes());
    }
}
