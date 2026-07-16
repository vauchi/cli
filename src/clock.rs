// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Test-clock seam for the CLI.
//!
//! The E2E harness runs each CLI command as a separate process and needs
//! deterministic control over wall-clock timestamps (clock-skew and
//! longitudinal scenarios). It sets [`ENV_VAR`] per invocation; this module
//! routes every CLI timestamp read through that override.
//!
//! Production behavior is unchanged when the variable is unset.

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use vauchi_core::clock::Clock;

/// Environment variable holding a Unix epoch (seconds, u64) that overrides
/// the wall clock. Test-only hook — production deployments must not set it.
pub const ENV_VAR: &str = "VAUCHI_TEST_CLOCK_EPOCH";

/// Clock that re-reads [`ENV_VAR`] on every `now()` call.
#[derive(Debug, Default)]
pub struct EnvClock;

impl Clock for EnvClock {
    fn now(&self) -> SystemTime {
        now()
    }
}

/// Drop-in replacement for `vauchi_core::clock::SystemClock::shared()`.
pub fn shared() -> Arc<dyn Clock> {
    Arc::new(EnvClock)
}

/// Current wall-clock time, honoring the [`ENV_VAR`] override.
///
/// Reads the variable on every call: unset falls back to `SystemTime::now()`
/// (production behavior), a valid u64 yields `UNIX_EPOCH + secs`, and a
/// malformed value panics — a bad override is a test-harness bug and must
/// never silently fall back to the real clock.
pub fn now() -> SystemTime {
    match std::env::var(ENV_VAR) {
        Err(std::env::VarError::NotPresent) => SystemTime::now(),
        Err(std::env::VarError::NotUnicode(_)) => {
            panic!("{ENV_VAR} is set but not valid Unicode; expected u64 Unix epoch seconds")
        }
        Ok(raw) => SystemTime::UNIX_EPOCH + Duration::from_secs(parse_epoch(&raw)),
    }
}

/// Parses an [`ENV_VAR`] value into Unix epoch seconds. Panics on
/// malformed input: a bad override is a test-harness bug and must never
/// silently fall back to the real clock.
fn parse_epoch(raw: &str) -> u64 {
    raw.parse().unwrap_or_else(|_| {
        panic!("{ENV_VAR} is set but malformed: expected u64 Unix epoch seconds, got {raw:?}")
    })
}

/// Current Unix epoch seconds, honoring the [`ENV_VAR`] override.
pub fn unix_seconds() -> u64 {
    now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Whether [`ENV_VAR`] is currently set (valid or not). Callers use this
/// to decide whether to thread the [`EnvClock`] into components whose
/// own clocks should follow the pin; a malformed value still counts as
/// pinned so the fail-fast panic surfaces at first use.
pub fn is_pinned() -> bool {
    std::env::var_os(ENV_VAR).is_some()
}

/// Test-only serialization for process-global [`ENV_VAR`] access.
#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Locks the env, tolerating poisoning from a panicking test.
#[cfg(test)]
pub(crate) fn env_lock() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Sets [`ENV_VAR`]. Callers must hold [`env_lock`] and keep an
/// [`EnvReset`] alive so the override is removed again on drop.
#[cfg(test)]
pub(crate) fn set_epoch(value: &str) {
    // SAFETY: callers serialize via env_lock; EnvReset removes the
    // variable again on drop, including during panic unwinding.
    unsafe { std::env::set_var(ENV_VAR, value) };
}

/// Removes [`ENV_VAR`] when dropped — also during panic unwinding, so a
/// failing test cannot leak the override into other tests.
#[cfg(test)]
pub(crate) struct EnvReset;

#[cfg(test)]
impl Drop for EnvReset {
    fn drop(&mut self) {
        // SAFETY: see set_epoch.
        unsafe { std::env::remove_var(ENV_VAR) };
    }
}

// INLINE_TEST_REQUIRED: Binary crate without lib.rs - tests cannot be external
#[cfg(test)]
mod tests {
    use super::*;

    // @internal
    #[test]
    fn unset_var_falls_back_to_system_time() {
        let _guard = env_lock();
        let _reset = EnvReset;

        let before = SystemTime::now();
        let got = now();
        let after = SystemTime::now();

        assert!(
            before <= got && got <= after,
            "unset {ENV_VAR} must return the real clock: got {got:?} outside [{before:?}, {after:?}]"
        );
    }

    // @internal
    #[test]
    fn set_valid_epoch_returns_injected_time() {
        let _guard = env_lock();
        let _reset = EnvReset;
        set_epoch("1700000000");

        let expected = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        assert_eq!(now(), expected);
        assert_eq!(unix_seconds(), 1_700_000_000);
        assert_eq!(shared().now(), expected, "EnvClock must read {ENV_VAR}");
    }

    /// Malformed values panic naming the variable and the bad value.
    ///
    /// Exercises `parse_epoch` directly rather than setting the process
    /// env: a malformed override must never be visible to other tests in
    /// this binary (they read the clock concurrently and would panic
    /// too), so the panic case stays out of the process-global env.
    // @internal
    #[test]
    #[should_panic(expected = "VAUCHI_TEST_CLOCK_EPOCH")]
    fn malformed_epoch_panics() {
        let _ = parse_epoch("abc");
    }
}
