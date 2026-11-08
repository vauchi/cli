// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{Context, Result, anyhow};
use vauchi_core::exchange::{DeviceLinkQR, DeviceLinkResponder, DeviceLinkResponse};
use vauchi_core::identity::DeviceRegistry;
use vauchi_core::storage::{FileKeyStorage, SecureStorage};
use vauchi_core::{DeviceSyncOrchestrator, Vauchi};
use zeroize::Zeroizing;

use crate::config::CliConfig;

pub(super) const PENDING_SIGNING_KEY: &str = "device_signing";

pub(super) fn pending_device_keys(config: &CliConfig) -> Result<FileKeyStorage> {
    Ok(FileKeyStorage::new(
        config.data_dir.join(".pending_device_join"),
        config.storage_key()?,
    ))
}

pub(super) fn restore_device_responder(
    keys: &dyn SecureStorage,
    qr: DeviceLinkQR,
    name: String,
) -> Result<DeviceLinkResponder> {
    let bytes = Zeroizing::new(
        keys.load_key(PENDING_SIGNING_KEY)?
            .context("No pending device signing key. Run 'vauchi device join' again.")?,
    );
    let seed = Zeroizing::new(<[u8; 32]>::try_from(bytes.as_slice()).map_err(|_| {
        anyhow!("Invalid pending device signing key. Run 'vauchi device join' again.")
    })?);
    Ok(DeviceLinkResponder::resume(qr, name, seed))
}

pub(super) fn adopt_link_response(
    config: &CliConfig,
    response: &DeviceLinkResponse,
    name: String,
) -> Result<()> {
    crate::commands::storage_replacement::replace(
        config,
        ".device-link-",
        |replacement| {
            replacement.adopt_device_link_response(response, name)?;
            Ok(())
        },
        |from, to| std::fs::rename(from, to),
    )
}

pub(crate) fn persist_updated_registry(
    vauchi: &Vauchi,
    registry: &DeviceRegistry,
    now: u64,
) -> Result<()> {
    let identity = vauchi
        .identity()
        .ok_or_else(|| anyhow!("No identity found"))?;
    DeviceSyncOrchestrator::persist_device_registry_change(
        vauchi.storage(),
        identity,
        registry,
        now,
    )
    .map_err(|error| anyhow!("Failed to persist linked-device registry: {error}"))
}

// INLINE_TEST_REQUIRED: Binary crate without lib.rs - tests cannot be external
#[cfg(test)]
mod tests {
    use super::persist_updated_registry;
    use vauchi_core::identity::DeviceInfo;
    use vauchi_core::sync::SyncItem;
    use vauchi_core::{DeviceSyncOrchestrator, Vauchi};

    // @internal
    #[test]
    fn cli_device_adoption_failure_preserves_existing_identity() {
        let directory = tempfile::tempdir().unwrap();
        let config = crate::config::CliConfig {
            data_dir: directory.path().to_path_buf(),
            relay_url: "ws://127.0.0.1:9".into(),
            ohttp_relay_url: None,
            ohttp_delayed: false,
            delayed_wait: crate::config::DelayedWait::default(),
            relay_anchor: None,
            raw: false,
        };
        let key = config.storage_key().unwrap();
        let mut original = Vauchi::new(
            vauchi_core::VauchiConfig::with_storage_path(config.storage_path())
                .with_storage_key(key.clone()),
        )
        .unwrap();
        original.create_identity("Existing").unwrap();
        let original_id = original.public_id().unwrap();
        let identity = original.identity().unwrap();
        let response = vauchi_core::exchange::DeviceLinkResponse::new(
            *identity.master_seed(),
            "Incoming".into(),
            1,
            identity.initial_device_registry(),
        );
        drop(original);
        super::adopt_link_response(&config, &response, "Second".into())
            .expect_err("response without a device key must fail adoption");
        let reloaded = Vauchi::new(
            vauchi_core::VauchiConfig::with_storage_path(config.storage_path())
                .with_storage_key(key),
        )
        .unwrap();
        assert_eq!(
            reloaded
                .public_id()
                .expect("original identity must survive"),
            original_id
        );
    }

    // @scenario: release_privacy_multidevice_certification.feature:Every active device can exchange and update
    #[test]
    fn cli_link_persistence_queues_expanded_registry_for_earlier_device() {
        let mut vauchi = Vauchi::in_memory().unwrap();
        vauchi.create_identity("Alice").unwrap();
        let identity = vauchi.identity().unwrap();
        let seed = *identity.master_seed();
        let phone = identity.create_device_info(1);
        let tablet = DeviceInfo::derive(&seed, 1, "Alice tablet".into(), 1);
        let laptop = DeviceInfo::derive(&seed, 2, "Alice laptop".into(), 1);

        let mut previous = identity.initial_device_registry();
        previous
            .add_device(tablet.to_registered(&seed), identity.signing_keypair())
            .unwrap();
        vauchi
            .storage()
            .device()
            .save_device_registry(&previous)
            .unwrap();
        let mut expanded = previous;
        expanded
            .add_device(laptop.to_registered(&seed), identity.signing_keypair())
            .unwrap();

        persist_updated_registry(&vauchi, &expanded, 2).unwrap();

        let stored = vauchi
            .storage()
            .device()
            .load_device_registry()
            .unwrap()
            .unwrap();
        let orchestrator = DeviceSyncOrchestrator::load(vauchi.storage(), phone, stored).unwrap();
        assert!(
            orchestrator
                .pending_for_device(tablet.device_id())
                .iter()
                .any(|item| matches!(item, SyncItem::DeviceRegistryChanged { .. }))
        );
    }
}
