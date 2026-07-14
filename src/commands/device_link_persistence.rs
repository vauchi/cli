// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{Result, anyhow};
use vauchi_core::identity::DeviceRegistry;
use vauchi_core::{DeviceSyncOrchestrator, Vauchi};

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
