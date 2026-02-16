// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Wire Protocol
//!
//! Re-exports shared protocol types from vauchi-core's simple_message module.
//! This eliminates duplicate type definitions between CLI and core.

// Re-export core protocol types with CLI-friendly names.
pub use vauchi_core::network::simple_message::{
    create_device_sync_ack, create_signed_handshake, create_simple_ack as create_ack,
    create_simple_envelope as create_envelope, decode_simple_message as decode_message,
    encode_simple_message as encode_message, LegacyExchangeMessage as ExchangeMessage,
    SimpleAckStatus as AckStatus, SimpleDeviceSyncMessage as DeviceSyncMessage,
    SimpleEncryptedUpdate as EncryptedUpdate, SimplePayload as MessagePayload,
};
