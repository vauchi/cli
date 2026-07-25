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

job_block() {
    awk -v job="$1" '
        $0 == job ":" { in_job = 1 }
        in_job && $0 != job ":" && /^[^[:space:]#]/ { exit }
        in_job { print }
    ' "$ci_config"
}

require_source_guard() {
    job=$1
    source_pattern=$2
    source_label=$3
    block=$(job_block "$job")
    guard_line=$(printf '%s\n' "$block" |
        grep -n "CI_PIPELINE_SOURCE == \"$source_pattern\"" |
        head -1 | cut -d: -f1 || true)
    default_line=$(printf '%s\n' "$block" |
        grep -n 'CI_COMMIT_BRANCH == \$CI_DEFAULT_BRANCH' |
        head -1 | cut -d: -f1 || true)

    if [ -n "$guard_line" ] &&
        [ -n "$default_line" ] &&
        [ "$guard_line" -lt "$default_line" ] &&
        printf '%s\n' "$block" |
        sed -n "${guard_line},$((guard_line + 1))p" |
        grep -q 'when: never'; then
        echo "PASS: $job excludes $source_label before the default branch"
    else
        echo "FAIL: $job must exclude $source_label before the default branch" >&2
        failures=$((failures + 1))
    fi
}

for job in auto-tag:version publish:package:cli pages github-mirror
do
    require_source_guard "$job" schedule schedules
    require_source_guard "$job" pipeline "triggered pipelines"
done

if [ "$failures" -ne 0 ]; then
    exit 1
fi
