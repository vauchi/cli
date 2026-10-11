// SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! `diag ohttp-probe` output is a contract: the hourly production probe
//! (`_private/infra/scripts/ohttp-probe.sh`) records a result only when it
//! finds "OHTTP forward hop OK:" (exit 0) or "OHTTP forward hop FAILED at ".

use super::{probe_report, probe_target};
use vauchi_core::api::RelayConfig;
use vauchi_core::network::ohttp_probe::{OhttpProbeFailure, OhttpProbeStep};

// @internal
#[test]
fn a_passing_probe_reports_ok_with_the_endpoint_and_exits_zero() {
    let (line, code) = probe_report("https://ohttp.vauchi.app", &Ok(()), "en");

    assert_eq!(line, "OHTTP forward hop OK: https://ohttp.vauchi.app");
    assert_eq!(code, 0);
}

// @internal
#[test]
fn a_failing_probe_names_the_step_and_exits_one() {
    let failure = OhttpProbeFailure {
        step: OhttpProbeStep::EncapsulatedRequest,
        detail: "HTTP 502".to_string(),
    };

    let (line, code) = probe_report("https://ohttp.vauchi.app", &Err(failure), "en");

    assert_eq!(
        line,
        "OHTTP forward hop FAILED at encapsulated request (https://ohttp.vauchi.app): HTTP 502"
    );
    assert_eq!(code, 1);
}

// @internal
#[test]
fn the_production_relay_is_probed_through_the_ohttp_host_with_the_pins_sync_uses() {
    let (url, pins, _) = probe_target("wss://relay.vauchi.app", None, None);

    assert_eq!(url, "https://ohttp.vauchi.app");
    assert_eq!(
        format!("{pins:?}"),
        format!("{:?}", RelayConfig::default().ohttp_endpoint_pins())
    );
}

// @internal
#[test]
fn an_explicit_ohttp_relay_is_probed_as_given() {
    let (url, _, _) = probe_target(
        "wss://relay.vauchi.app",
        Some("https://ohttp.example.org"),
        None,
    );

    assert_eq!(url, "https://ohttp.example.org");
}

// @internal
#[test]
fn a_self_hosted_relay_is_probed_on_its_own_host() {
    let (url, _, _) = probe_target("wss://relay.example.org", None, None);

    assert!(url.contains("relay.example.org"), "{url}");
    assert!(!url.contains("vauchi.app"), "{url}");
}

// The probe checks the gateway's signed key record under the relay's
// anchor (core v0.80.0, private#288): the production relay's own, or the
// one given with --relay-anchor.
// @internal
#[test]
fn the_production_relay_is_probed_under_its_own_anchor() {
    let (_, _, anchor) = probe_target("wss://relay.vauchi.app", None, None);

    let expected = RelayConfig::default()
        .ohttp_trust_anchor()
        .expect("the production relay carries an anchor");
    assert_eq!(anchor, Some(expected));
}

// @internal
#[test]
fn a_given_relay_anchor_is_used_for_any_relay() {
    let (_, _, anchor) = probe_target("wss://relay.example.org", None, Some([0x5a; 32]));

    assert_eq!(anchor, Some([0x5a; 32]));
}

// @internal
#[test]
fn a_self_hosted_relay_without_an_anchor_has_none() {
    let (_, _, anchor) = probe_target("wss://relay.example.org", None, None);

    assert_eq!(anchor, None);
}
