#!/usr/bin/env bash
# US-008 verification — confirms the smoke-scope boundaries are
# intact: no secrets or server-side APIs leak into client-reachable
# code, no cancelled items get implemented accidentally, no auth
# gets bolted on, no mobile build artifacts, Dashboard reads from
# the hardcoded FIXTURES list (no live use_resource).
#
# Each acceptance criterion from US-008 maps to one labeled check
# below. The script exits 0 only when every check passes. Failing
# checks print the underlying grep output so triage is one
# keystroke away.
#
# Usage:
#   ./scripts/verify-us-008.sh
#
# Exit code 0 = all 8 acceptance criteria pass.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT/alps-ui"

LOG_DIR="${REPO_ROOT}/target/us008-verify"
mkdir -p "$LOG_DIR"

# ─────────────────────────────────────────────────────────────────────
# Helpers
# ─────────────────────────────────────────────────────────────────────

assert_zero() {
    local label="$1"; shift
    local file="$1"; shift
    local captured
    captured=$("$@" 2>/dev/null || true)
    if [ -z "$captured" ]; then
        echo "  PASS: $label"
        return 0
    fi
    echo "  FAIL: $label"
    echo "$captured" | head -20
    return 1
}

assert_contains() {
    local label="$1"; shift
    local needle="$1"; shift
    if grep -qF -- "$needle" "$@"; then
        echo "  PASS: $label"
        return 0
    fi
    echo "  FAIL: $label (needle '$needle' not found)"
    return 1
}

assert_build() {
    local label="$1"; shift
    local features="$1"; shift
    local LOG="$LOG_DIR/$1.log"; shift
    if cargo build --bin alps-ui $features >"$LOG" 2>&1; then
        echo "  PASS: $label"
        return 0
    fi
    echo "  FAIL: $label"
    tail -20 "$LOG"
    return 1
}

# ─────────────────────────────────────────────────────────────────────
# Acceptance #1: no std::process::Command outside #[cfg(feature = "server")]
# ─────────────────────────────────────────────────────────────────────

echo "--- US-008 #1: std::process::Command outside server-cfg ---"
assert_zero \
    "zero non-server hits for std::process::Command" \
    "${LOG_DIR}/acceptance-1-process-command.log" \
    bash -c "grep -rn 'std::process::Command' src/ | grep -v '\\[cfg(feature = \"server\")\\]'"

# ─────────────────────────────────────────────────────────────────────
# Acceptance #2: no std::fs:: outside #[cfg(feature = "server")]
# ─────────────────────────────────────────────────────────────────────

echo
echo "--- US-008 #2: std::fs:: outside server-cfg ---"
assert_zero \
    "zero non-server hits for std::fs::" \
    "${LOG_DIR}/acceptance-2-fs.log" \
    bash -c "grep -rn 'std::fs::' src/ | grep -v '\\[cfg(feature = \"server\")\\]'"

# ─────────────────────────────────────────────────────────────────────
# Acceptance #3: task_log_stream NOT implemented (no SSE)
# ─────────────────────────────────────────────────────────────────────

echo
echo "--- US-008 #3: SSE / EventSource absent ---"
assert_zero \
    "zero hits for ServerEvents | axum::response::sse | EventSource" \
    "${LOG_DIR}/acceptance-3-sse.log" \
    grep -rn 'ServerEvents\|axum::response::sse\|EventSource' src/

# ─────────────────────────────────────────────────────────────────────
# Acceptance #4: task_cancel NOT implemented (no SIGTERM dispatch)
# ─────────────────────────────────────────────────────────────────────

echo
echo "--- US-008 #4: signal dispatch absent ---"
assert_zero \
    "zero hits for libc::kill | signal_hook | tokio::signal" \
    "${LOG_DIR}/acceptance-4-signals.log" \
    grep -rn 'libc::kill\|signal_hook\|tokio::signal' src/

# ─────────────────────────────────────────────────────────────────────
# Acceptance #5: Settings page is a stub
# ─────────────────────────────────────────────────────────────────────

echo
echo "--- US-008 #5: Settings page is a stub ---"
assert_contains \
    "Settings page renders 'coming in v2' copy" \
    "coming in v2" \
    src/pages/settings.rs

# ─────────────────────────────────────────────────────────────────────
# Acceptance #6: no dx bundle --mobile invocation
# ─────────────────────────────────────────────────────────────────────

echo
echo "--- US-008 #6: no mobile bundle in scripts/ ---"
assert_zero \
    "zero hits for 'dx bundle ... mobile' or 'mobile ... --release'" \
    "${LOG_DIR}/acceptance-6-mobile.log" \
    bash -c "grep -rn 'dx bundle.*mobile\\|mobile.*--release' scripts/"

# ─────────────────────────────────────────────────────────────────────
# Acceptance #7: no authentication code
# ─────────────────────────────────────────────────────────────────────

echo
echo "--- US-008 #7: no authentication code ---"
# Look for actual functional auth code (not comments mentioning 'no auth').
# Patterns: fn auth..., fn login..., fn verify_token..., use ... ::Authentication
assert_zero \
    "zero functional auth/login/verify_token hits" \
    "${LOG_DIR}/acceptance-7-auth.log" \
    grep -rEn 'fn (auth|login|verify_token|authenticate)\b|use .*::(Authentication|Authorization|Jwt|JsonWebToken)' src/

# ─────────────────────────────────────────────────────────────────────
# Acceptance #8: Dashboard reads from FIXTURES, no live use_resource
# ─────────────────────────────────────────────────────────────────────

echo
echo "--- US-008 #8: Dashboard hardcoded FIXTURES (no use_resource call) ---"
# Functional check: no `use_resource` invocation in dashboard.rs body (only
# mentioned in doc-comments stating it IS NOT used yet). The criterion text
# is "no use_resource calling tasks_list from the Dashboard page" — a
# semantic check.
HITS=$(grep -n '^[^/]*use_resource' src/pages/dashboard.rs 2>/dev/null || true)
if [ -z "$HITS" ]; then
    echo "  PASS: no use_resource(...) invocation in dashboard.rs"
else
    echo "  FAIL: use_resource present in dashboard.rs:"
    echo "$HITS"
    exit 1
fi
if grep -q 'use crate::fixtures::FIXTURES' src/pages/dashboard.rs; then
    echo "  PASS: dashboard.rs imports FIXTURES"
else
    echo "  FAIL: dashboard.rs does NOT import FIXTURES"
    exit 1
fi

# ─────────────────────────────────────────────────────────────────────
# Acceptance #1+2 (BUILD verification): both build flavors exit 0
# ─────────────────────────────────────────────────────────────────────

echo
echo "--- US-008 #9a: cargo build --bin alps-ui ---"
assert_build "cargo build --bin alps-ui (default = web)" \
    "" \
    acceptance-9a-build-default

echo
echo "--- US-008 #9b: cargo build --bin alps-ui --features fullstack ---"
assert_build "cargo build --bin alps-ui --features fullstack" \
    "--features fullstack" \
    acceptance-9b-build-fullstack

# ─────────────────────────────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────────────────────────────

echo
echo "================================================================"
echo "  US-008 verification: all 8 acceptance criteria pass."
echo "  Logs: $LOG_DIR"
echo "================================================================"
exit 0
