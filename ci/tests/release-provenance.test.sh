#!/bin/sh
# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
#
# SPDX-License-Identifier: GPL-3.0-or-later

set -eu

ci_config="${1:-.gitlab-ci.yml}"
failures=0

if awk '
    /^build:release:/ { in_job = 1; next }
    in_job && /^[^[:space:]]/ { exit }
    in_job && /cargo build --release --locked/ { found = 1 }
    END { exit !found }
' "$ci_config"; then
    echo "PASS: release build uses the committed Cargo.lock"
else
    echo "FAIL: release build must use cargo build --release --locked" >&2
    failures=$((failures + 1))
fi

if grep -q '^check:core-version:' "$ci_config"; then
    echo "FAIL: release provenance must not depend on incidental binary strings" >&2
    failures=$((failures + 1))
else
    echo "PASS: release provenance does not inspect incidental binary strings"
fi

if [ "$failures" -ne 0 ]; then
    exit 1
fi
