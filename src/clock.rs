// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Test-clock seam for the dedicated CLI E2E binary.
//!
//! The E2E harness runs each CLI command as a separate process and needs
//! deterministic control over wall-clock timestamps (clock-skew and
//! longitudinal scenarios). It sets [`ENV_VAR`] per invocation; this module
//! routes every CLI timestamp read through that override when compiled with
//! the `e2e-test-clock` feature. Shipping builds always use the system clock.

use std::sync::Arc;
use std::time::SystemTime;

#[cfg(feature = "e2e-test-clock")]
use std::time::Duration;

use vauchi_core::clock::Clock;

/// Environment variable holding a Unix epoch (seconds, u64) for the dedicated
/// E2E binary. Default builds deliberately ignore it.
#[cfg(any(test, feature = "e2e-test-clock"))]
pub const ENV_VAR: &str = "VAUCHI_TEST_CLOCK_EPOCH";

/// Clock that re-reads [`ENV_VAR`] only in the E2E-feature build.
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

/// Current wall-clock time. The E2E-feature build honors [`ENV_VAR`]; default
/// builds always use `SystemTime::now()` so an environment variable cannot
/// bypass a destructive-operation grace period.
pub fn now() -> SystemTime {
    #[cfg(not(feature = "e2e-test-clock"))]
    return SystemTime::now();

    #[cfg(feature = "e2e-test-clock")]
    time_from_env(std::env::var(ENV_VAR), SystemTime::now())
}

#[cfg(feature = "e2e-test-clock")]
fn time_from_env(value: Result<String, std::env::VarError>, fallback: SystemTime) -> SystemTime {
    match value {
        Err(std::env::VarError::NotPresent) => fallback,
        Err(std::env::VarError::NotUnicode(_)) => {
            panic!("{ENV_VAR} is set but not valid Unicode; expected u64 Unix epoch seconds")
        }
        Ok(raw) => SystemTime::UNIX_EPOCH + Duration::from_secs(parse_epoch(&raw)),
    }
}

/// Parses an [`ENV_VAR`] value into Unix epoch seconds. Panics on
/// malformed input: a bad override is a test-harness bug and must never
/// silently fall back to the real clock.
#[cfg(feature = "e2e-test-clock")]
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

/// Whether this E2E-feature build has enabled the test clock.
pub fn is_pinned() -> bool {
    #[cfg(feature = "e2e-test-clock")]
    return std::env::var_os(ENV_VAR).is_some();

    #[cfg(not(feature = "e2e-test-clock"))]
    false
}

// INLINE_TEST_REQUIRED: Binary crate without lib.rs - tests cannot be external
#[cfg(test)]
mod tests {
    use super::*;

    // @internal
    #[cfg(feature = "e2e-test-clock")]
    #[test]
    fn unset_var_falls_back_to_system_time() {
        let fallback = SystemTime::UNIX_EPOCH + Duration::from_secs(42);
        assert_eq!(
            time_from_env(Err(std::env::VarError::NotPresent), fallback),
            fallback
        );
    }

    /// A shipping CLI must never let a caller bypass a destructive-operation
    /// grace period through its process environment.
    // @internal
    #[cfg(not(feature = "e2e-test-clock"))]
    #[test]
    fn production_build_ignores_clock_override() {
        let before = SystemTime::now();
        let got = now();
        let after = SystemTime::now();

        assert!(
            before <= got && got <= after,
            "default build must ignore {ENV_VAR}: got {got:?} outside [{before:?}, {after:?}]"
        );
        assert!(!is_pinned(), "default build must not enable the test clock");
    }

    // @internal
    #[cfg(feature = "e2e-test-clock")]
    #[test]
    fn set_valid_epoch_returns_injected_time() {
        let expected = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        assert_eq!(
            time_from_env(Ok("1700000000".to_string()), SystemTime::UNIX_EPOCH),
            expected
        );
    }

    /// Malformed values panic naming the variable and the bad value.
    ///
    /// Exercises `parse_epoch` directly rather than setting the process
    /// env: a malformed override must never be visible to other tests in
    /// this binary (they read the clock concurrently and would panic
    /// too), so the panic case stays out of the process-global env.
    // @internal
    #[cfg(feature = "e2e-test-clock")]
    #[test]
    #[should_panic(expected = "VAUCHI_TEST_CLOCK_EPOCH")]
    fn malformed_epoch_panics() {
        let _ = parse_epoch("abc");
    }
}
