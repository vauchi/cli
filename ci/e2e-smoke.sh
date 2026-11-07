#!/bin/sh
# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# e2e's smoke suite on this CLI merge request (vauchi/private#1118).
# Core's merge gate runs the suite on e2e's in-process Core device, so a
# Core change the CLI must follow can merge first; this job is where the
# CLI itself is tested against the suite. It builds the CLI from this
# checkout, with the Core it pins (E2E_CLI_DIR), and e2e's relays at their
# pins, then runs smoke_ on the CLI device.
#
# Usage: ci/e2e-smoke.sh <e2e-checkout> <cli-checkout>

set -eu

e2e=${1:?usage: e2e-smoke.sh <e2e-checkout> <cli-checkout>}
cli=${2:?cli checkout}

E2E_DEVICE_BACKEND=cli
export E2E_DEVICE_BACKEND

E2E_CLI_DIR="$cli" sh "$e2e/ci/build-pinned-binaries.sh" "$e2e/.bin"
cd "$e2e"
E2E_BIN_DIR="$e2e/.bin" exec cargo nextest run --profile ci --run-ignored default -j 4 -- smoke_
