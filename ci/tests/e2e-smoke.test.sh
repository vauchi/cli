#!/bin/sh
# SPDX-FileCopyrightText: 2026 Mattia Egloff <mattia.egloff@pm.me>
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# A CLI merge request runs e2e's smoke suite on the CLI it builds
# (vauchi/private#1118). Checks the contract:
#
# - test:e2e-smoke clones e2e and runs ci/e2e-smoke.sh on this checkout;
# - ci/e2e-smoke.sh builds the CLI from this checkout (E2E_CLI_DIR), not
#   e2e's pinned commit, and runs smoke_ on the CLI device.
#
# Fakes stand in for e2e's build script and cargo.
#
# Usage: ci/tests/e2e-smoke.test.sh [ci-config]

set -eu
set -f

ci_config=${1:-.gitlab-ci.yml}
job="test:e2e-smoke"
failures=0
fail() {
    echo "FAIL: $1" >&2
    failures=$((failures + 1))
}

block=$(awk -v job="$job" '
    $0 == job ":" { in_job = 1; next }
    in_job && /^[^[:space:]#]/ { exit }
    in_job { print }
' "$ci_config")

if [ -z "$block" ]; then
    fail "$job is not in $ci_config"
else
    for step in \
        'gitlab.com/vauchi/e2e.git' \
        'sh ci/e2e-smoke.sh "$E2E_DIR" "$CI_PROJECT_DIR"' \
        'merge_request_event'; do
        if printf '%s\n' "$block" | grep -qF -- "$step"; then
            echo "PASS: $job has $step"
        else
            fail "$job must have: $step"
        fi
    done
fi

fake=$(mktemp -d)
trap 'rm -rf "$fake"' EXIT
mkdir -p "$fake/e2e/ci" "$fake/bin" "$fake/cli"
cat > "$fake/e2e/ci/build-pinned-binaries.sh" <<'EOF'
#!/bin/sh
echo "build E2E_CLI_DIR=${E2E_CLI_DIR:-} backend=${E2E_DEVICE_BACKEND:-} bin=$1" >> "$FAKE_LOG"
EOF
cat > "$fake/bin/cargo" <<'EOF'
#!/bin/sh
echo "cargo backend=${E2E_DEVICE_BACKEND:-} bin=${E2E_BIN_DIR:-} args=$*" >> "$FAKE_LOG"
EOF
chmod +x "$fake/bin/cargo"
: > "$fake/log"

FAKE_LOG="$fake/log" PATH="$fake/bin:$PATH" sh ci/e2e-smoke.sh "$fake/e2e" "$fake/cli" > "$fake/out" 2>&1 ||
    fail "ci/e2e-smoke.sh failed: $(tail -3 "$fake/out")"

grep -qxF "build E2E_CLI_DIR=$fake/cli backend=cli bin=$fake/e2e/.bin" "$fake/log" &&
    echo "PASS: the CLI is built from this checkout" ||
    fail "the CLI must be built from this checkout: $(cat "$fake/log")"
grep -q "^cargo backend=cli bin=$fake/e2e/.bin args=nextest run .*-- smoke_$" "$fake/log" &&
    echo "PASS: smoke_ runs on the CLI device with those binaries" ||
    fail "smoke_ must run on the CLI device: $(cat "$fake/log")"

if [ "$failures" -gt 0 ]; then
    echo "$failures failure(s)" >&2
    exit 1
fi
echo "OK: CLI merge requests run e2e smoke on their own CLI"
