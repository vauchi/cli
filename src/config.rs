// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! CLI Configuration

use std::path::{Path, PathBuf};

use anyhow::Result;
use vauchi_core::{Identity, IdentityBackup, SymmetricKey};

#[cfg(feature = "secure-storage")]
use vauchi_core::storage::secure::{PlatformKeyring, SecureStorage};

#[cfg(not(feature = "secure-storage"))]
use vauchi_core::storage::secure::{FileKeyStorage, SecureStorage};

/// Legacy hardcoded password used before per-installation backup passwords.
const LEGACY_BACKUP_PASSWORD: &str = "vauchi-local-storage";

/// Writes data to a file with restrictive permissions (0o600 on Unix).
///
/// Prevents other users on shared systems from reading sensitive files
/// like pending device link keys, recovery claims, and tracker state.
pub fn write_restricted(path: &Path, data: impl AsRef<[u8]>) -> anyhow::Result<()> {
    use anyhow::Context;
    std::fs::write(path, data).with_context(|| format!("Failed to write {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("Failed to set permissions on {}", path.display()))?;
    }
    Ok(())
}

/// CLI configuration.
#[derive(Debug, Clone)]
pub struct CliConfig {
    /// Data directory for storage.
    pub data_dir: PathBuf,
    /// Relay server URL.
    pub relay_url: String,
    /// Output raw JSON instead of formatted text.
    pub raw: bool,
}

/// Key name used for SecureStorage (non-keychain path).
const KEY_NAME: &str = "storage_key";

/// Derives a stable per-install keychain key name from the install_id stored
/// at `<data_dir>/install_id`.
///
/// The install_id moves with the data directory on rename — so the OS keychain
/// entry stays reachable even if the user relocates `data_dir`. Two installs
/// with distinct data directories get distinct ids and distinct keychain
/// entries.
#[cfg_attr(not(feature = "secure-storage"), allow(dead_code))]
fn keychain_key_name(data_dir: &std::path::Path) -> Result<String> {
    let install_id = vauchi_core::install_id::read_or_create_install_id(data_dir)?;
    Ok(format!("storage_key_{install_id}"))
}

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

    write_restricted(&key_path, key.as_bytes())?;

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

    write_restricted(&password_path, &password)?;

    Ok(password)
}

impl CliConfig {
    /// Returns the storage path for Vauchi data.
    pub fn storage_path(&self) -> PathBuf {
        self.data_dir.join("vauchi.db")
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
    /// When the `secure-storage` feature is enabled, uses the OS keychain
    /// with a key name derived from the install_id stored next to the data
    /// directory. Otherwise, falls back to encrypted file storage.
    #[allow(unused_variables)]
    pub fn storage_key(&self) -> Result<SymmetricKey> {
        #[cfg(feature = "secure-storage")]
        {
            let storage = PlatformKeyring::new("vauchi-cli");
            let key_name = keychain_key_name(&self.data_dir)?;

            match storage.load_key(&key_name) {
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
                        .save_key(&key_name, key.as_bytes())
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
            raw: false,
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
            raw: false,
        };

        // First call creates key
        let key1 = config.storage_key().expect("should create key");

        // Second call should return the same key
        let key2 = config.storage_key().expect("should load key");

        assert_eq!(key1.as_bytes(), key2.as_bytes());
    }

    // @internal
    #[test]
    fn keychain_key_name_is_stable_across_calls() {
        let temp_dir = tempdir().unwrap();
        let name1 = keychain_key_name(temp_dir.path()).unwrap();
        let name2 = keychain_key_name(temp_dir.path()).unwrap();
        assert_eq!(name1, name2);
    }

    // @internal
    #[test]
    fn keychain_key_name_survives_data_dir_rename() {
        // Regression: pre-fix, the key name was derived from `fnv1a(data_dir)`,
        // so renaming the data directory orphaned the OS keychain entry. Now
        // the name is derived from the install_id file, which moves with the
        // data — rename is invisible to the keychain lookup.
        let parent = tempdir().unwrap();
        let original = parent.path().join("original");
        std::fs::create_dir_all(&original).unwrap();
        let name_before = keychain_key_name(&original).unwrap();

        let renamed = parent.path().join("renamed");
        std::fs::rename(&original, &renamed).unwrap();
        let name_after = keychain_key_name(&renamed).unwrap();

        assert_eq!(name_before, name_after);
    }

    // @internal
    #[test]
    fn keychain_key_name_differs_per_data_dir() {
        let dir_a = tempdir().unwrap();
        let dir_b = tempdir().unwrap();
        let name_a = keychain_key_name(dir_a.path()).unwrap();
        let name_b = keychain_key_name(dir_b.path()).unwrap();
        assert_ne!(name_a, name_b);
    }

    // === Backup Password Tests ===
    // Trace: codebase-review-tracker item #27

    #[test]
    fn test_backup_password_is_not_hardcoded() {
        let temp_dir = tempdir().unwrap();
        let config = CliConfig {
            data_dir: temp_dir.path().to_path_buf(),
            relay_url: "ws://localhost:8080".to_string(),
            raw: false,
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
            raw: false,
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
            raw: false,
        };
        let config2 = CliConfig {
            data_dir: temp2.path().to_path_buf(),
            relay_url: "ws://localhost:8080".to_string(),
            raw: false,
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
            raw: false,
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
            raw: false,
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
            raw: false,
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

    #[cfg(feature = "secure-storage")]
    #[test]
    fn test_keychain_key_name_differs_per_data_dir() {
        use std::path::PathBuf;

        let key1 = keychain_key_name(&PathBuf::from("/tmp/vauchi-test-1"));
        let key2 = keychain_key_name(&PathBuf::from("/tmp/vauchi-test-2"));
        let key3 = keychain_key_name(&PathBuf::from("/tmp/vauchi-test-1"));

        // Different paths produce different key names
        assert_ne!(key1, key2);
        // Same path produces the same key name
        assert_eq!(key1, key3);
        // All start with the expected prefix
        assert!(key1.starts_with("storage_key_"));
        assert!(key2.starts_with("storage_key_"));
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
            raw: false,
        };
        let key1 = config1.storage_key().expect("should create key");

        // Second config instance with same data_dir loads same key
        let config2 = CliConfig {
            data_dir,
            relay_url: "ws://localhost:8080".to_string(),
            raw: false,
        };
        let key2 = config2.storage_key().expect("should load key");

        assert_eq!(key1.as_bytes(), key2.as_bytes());
    }
}
