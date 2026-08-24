#!/usr/bin/env bash
# US-007 verification — runs the full smoke build + clippy + dx serve
# acceptance suite end-to-end so a future smoke (or a codex iteration)
# can re-verify "did the deliverables still produce a runnable UI?"
# in one shot, without re-discovering the `--platform=server` /
# `--features=server` workaround for the wasm32 tokio/mio
# incompatibility documented in progress.txt.
#
# Each acceptance criterion from US-007 is mapped to a labeled
# check with a clear PASS / FAIL line. The script exits 0 only if
# every check passes. Failing checks print the underlying command
# output so triage is one keystroke away.
#
# Why this script exists (vs re-running the verification ad-hoc):
# - US-007's acceptance gate is "build + clippy + dx serve + 8 fixture
#   states served" — six separate checks across two build flavors.
#   Recreating the run from a fresh checkout is tedious and error-prone.
# - The dx serve has to run on a non-default port (default 5174 is
#   frequently occupied in the smoke environment by another smoke's
#   vite instance), and the right `--platform` flag depends on whether
#   alps-core's `tokio = "full"` blocks wasm32 (it does — see progress.txt).
# - Future codex iterations land on US-008 / follow-ups and benefit
#   from a one-shot regression check that says "US-007 is still green."
#
# Usage:
#   ./scripts/verify-us-007.sh [--port <PORT>]   # default: 5274
#
# Exit code 0 = all 6 acceptance criteria pass.

set -euo pipefail

# ─────────────────────────────────────────────────────────────────────
# Parameters
# ─────────────────────────────────────────────────────────────────────

PORT="${PORT_OVERRIDE:-5274}"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --port) PORT="$2"; shift 2 ;;
        --help|-h)
            sed -n '2,/^$/p' "$0" | sed 's/^# \?//'
            exit 0
            ;;
        *) echo "error: unknown argument: $1" >&2; exit 2 ;;
    esac
done

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

LOG_DIR="${REPO_ROOT}/target/us007-verify"
mkdir -p "$LOG_DIR"

# ─────────────────────────────────────────────────────────────────────
# Helpers
# ─────────────────────────────────────────────────────────────────────

assert_cmd() {
    local label="$1"; shift
    local logfile="$LOG_DIR/$1.log"; shift
    local expect_pass="$1"; shift
    echo "--- ${label} ---"
    if "$@" >"$logfile" 2>&1; then
        if [ "$expect_pass" = "pass" ]; then
            echo "  PASS"
            return 0
        else
            echo "  FAIL (command exited 0 unexpectedly)"
            tail -20 "$logfile"
            return 1
        fi
    else
        if [ "$expect_pass" = "fail" ]; then
            echo "  PASS (expected failure)"
            return 0
        else
            echo "  FAIL"
            tail -20 "$logfile"
            return 1
        fi
    fi
}

HTML_TMP=""
cleanup_serve() {
    if [ -n "${SERVE_PID:-}" ]; then
        kill "$SERVE_PID" 2>/dev/null || true
        pgrep -g "$(ps -o pgid= -p "$SERVE_PID" 2>/dev/null | tr -d ' ')" 2>/dev/null \
            | xargs -r kill 2>/dev/null || true
        wait "$SERVE_PID" 2>/dev/null || true
        SERVE_PID=""
    fi
    if [ -n "$HTML_TMP" ] && [ -f "$HTML_TMP" ]; then
        rm -f "$HTML_TMP"
        HTML_TMP=""
    fi
}
trap cleanup_serve EXIT

# ─────────────────────────────────────────────────────────────────────
# Acceptance #1: cargo build --bin alps-ui
# ─────────────────────────────────────────────────────────────────────

assert_cmd "US-007 #1: cargo build --bin alps-ui" \
    acceptance-1-build-default \
    pass \
    cargo build --bin alps-ui

# ─────────────────────────────────────────────────────────────────────
# Acceptance #2: cargo build --bin alps-ui --features fullstack
# ─────────────────────────────────────────────────────────────────────

assert_cmd "US-007 #2: cargo build --bin alps-ui --features fullstack" \
    acceptance-2-build-fullstack \
    pass \
    cargo build --bin alps-ui --features fullstack

# ─────────────────────────────────────────────────────────────────────
# Acceptance #3: cargo clippy (zero alps-ui warnings)
# ─────────────────────────────────────────────────────────────────────

echo "--- US-007 #3: cargo clippy --bin alps-ui -- -D warnings ---"
LOG="$LOG_DIR/acceptance-3-clippy.log"
if cargo clippy --bin alps-ui --no-deps -- -D warnings >"$LOG" 2>&1; then
    UI_WARN_COUNT=$(grep -E -- '--\> .*alps-ui/src/' "$LOG" 2>/dev/null | wc -l; true)
    if [ "$UI_WARN_COUNT" = "0" ]; then
        echo "  PASS (zero alps-ui src warnings; the alps-core path-dep"
        echo "    warnings from its own crate are out of scope per US-001's"
        echo "    learned pattern.)"
    else
        echo "  FAIL ($UI_WARN_COUNT alps-ui source warnings)"
        grep -E -- '--\> .*alps-ui/src/' "$LOG" | head -20
        exit 1
    fi
else
    echo "  FAIL"
    tail -30 "$LOG"
    exit 1
fi

# ─────────────────────────────────────────────────────────────────────
# Acceptance #4-6: dx serve + curl checks
#
# The default `dx serve --platform=web` builds the wasm32 client, which
# fails to compile because alps-core declares `tokio = "full"` and the
# `net` / `process` / `signal` features trip tokio 1.53+'s wasm
# `compile_error!` (and mio's `net` trips its wasm-unsupported error).
# `--platform=server --features=server` builds the native target and
# uses `dioxus-liveview` (SSR), which renders the full Dashboard into
# the served HTML so curl sees every fixture label.
# ─────────────────────────────────────────────────────────────────────

if [ -z "$(command -v dx 2>/dev/null)" ]; then
    echo "--- US-007 acceptance gate: 'dx' not on PATH ---"
    echo "  FAIL: dx (dioxus-cli 0.7.10) is not installed."
    echo "  Install with: cargo install dioxus-cli --locked --version 0.7.10"
    echo "  US-007 acceptance criterion #7 requires a LOUD failure here,"
    echo "  not a silent fallback."
    exit 2
fi

# Check the chosen port is free; if 5174 is occupied (often, by another
# smoke's vite), the user can override with --port.
if ss -tlnp 2>/dev/null | grep -q ":$PORT "; then
    echo "--- US-007 #4-#6: dx serve (port $PORT) ---"
    echo "  FAIL: port $PORT already in use; pass --port <free>."
    exit 1
fi

echo "--- US-007 #4-#6: dx serve --platform=server --features=server on port $PORT ---"
LOG="$LOG_DIR/acceptance-4-serve.log"
timeout 480 dx serve --port "$PORT" --platform server --package alps-ui --features server \
    >"$LOG" 2>&1 &
SERVE_PID=$!

# Wait for the port to bind. CI runners (where this script first runs
# against a fresh cargo cache) can take 90s+ for dioxus-cli's first
# compile + axum to register routes. 120s is enough headroom.
for _ in $(seq 1 120); do
    sleep 1
    if ss -tlnp 2>/dev/null | grep -q "127.0.0.1:$PORT"; then
        break
    fi
done

# After bind, axum needs another moment to register the server-fn
# endpoints. On the first request, dioxus-cli may also proxy through
# a separate wasm-dev-server that needs its own warm-up window.
# Poll the index URL up to 30 times, accepting HTTP 200 OR a
# "backend not ready" page (which dx serves while the wasm dev
# server is still compiling). We only fail if 30 consecutive polls
# all return connection-refused or 500.
if ! ss -tlnp 2>/dev/null | grep -q "127.0.0.1:$PORT"; then
    echo "  FAIL: dx serve did not bind 127.0.0.1:$PORT within 120s."
    tail -50 "$LOG"
    cleanup_serve
    exit 1
fi
echo "  dx serve bound 127.0.0.1:$PORT"

HTML_TMP="$(mktemp)"
HTTP_CODE=""
for attempt in $(seq 1 30); do
    HTTP_CODE=$(curl -s -o "$HTML_TMP" -w "%{http_code}" "http://127.0.0.1:$PORT/" || echo "curl-failed")
    if [ "$HTTP_CODE" = "200" ]; then
        break
    fi
    sleep 2
done
if [ "$HTTP_CODE" != "200" ]; then
    echo "  FAIL: curl returned HTTP $HTTP_CODE after 30 attempts"
    head -20 "$HTML_TMP"
    cleanup_serve
    exit 1
fi

# Acceptance #4: HTTP 200 + dashboard route reachable.
if ! grep -qi dashboard "$HTML_TMP"; then
    echo "  FAIL: 'dashboard' not found in served HTML"
    head -20 "$HTML_TMP"
    cleanup_serve
    exit 1
fi
echo "  PASS #4: HTTP 200, dashboard route rendered"

# Acceptance #4b (added 2026-08-24, smoke-A2 M0-0c): every <link rel=stylesheet>
# href in the served HTML must resolve to HTTP 200. Closes the verification
# gap that let the SSR-mode unstyled-HTML defect ship on smoke #1 — the
# original #4 checked that <link> tags EXISTED but never verified the
# referenced CSS files actually load. Symptom was: Dioxus SSR ships an
# empty <head> in `--platform server` mode unless main.rs injects
# `document::Stylesheet { href: asset!(...) }`, in which case the
# content-hashed URL must be reachable on the same port.
STYLE_HREFS=$(grep -oE '<link[^>]*rel="stylesheet"[^>]*href="[^"]+"' "$HTML_TMP" \
    | grep -oE 'href="[^"]+"' | sed 's/href="//;s/"$//')
if [ -z "$STYLE_HREFS" ]; then
    echo "  FAIL #4b: no <link rel=\"stylesheet\"> tags found in served HTML"
    echo "    SSR's default index.html ships an empty <head>; main.rs must"
    echo "    inject document::Stylesheet to get CSS in --platform server mode."
    cleanup_serve
    exit 1
fi
CSS_FAIL=0
while IFS= read -r href; do
    [ -z "$href" ] && continue
    # Resolve relative hrefs against the served origin.
    case "$href" in
        http*) url="$href" ;;
        /*)    url="http://127.0.0.1:$PORT$href" ;;
        *)     url="http://127.0.0.1:$PORT/$href" ;;
    esac
    code=$(curl -s -o /dev/null -w "%{http_code}" "$url" || echo "curl-failed")
    if [ "$code" != "200" ]; then
        echo "  FAIL #4b: stylesheet $url returned HTTP $code"
        CSS_FAIL=1
    else
        size=$(curl -s "$url" | wc -c)
        echo "  PASS #4b: $url (HTTP 200, $size bytes)"
    fi
done <<< "$STYLE_HREFS"
if [ "$CSS_FAIL" = "1" ]; then
    cleanup_serve
    exit 1
fi

# Acceptance #5: served HTML contains all 8 fixture state labels.
STATES="Running|Idle|Planned|Implemented|Reviewed|Done|Rejected|Failed"
UNIQUE=$(grep -oE "$STATES" "$HTML_TMP" | sort -u | wc -l)
if [ "$UNIQUE" != "8" ]; then
    echo "  FAIL #5: served HTML contains $UNIQUE unique state labels (expected 8)."
    grep -oE "$STATES" "$HTML_TMP" | sort -u
    cleanup_serve
    exit 1
fi
echo "  PASS #5: served HTML contains all 8 state labels"

# Acceptance #6: dx serve background process is killed cleanly at end.
cleanup_serve
sleep 2
if ss -tlnp 2>/dev/null | grep -q "127.0.0.1:$PORT"; then
    echo "  FAIL #6: port $PORT still bound after cleanup"
    exit 1
fi
echo "  PASS #6: dx serve killed cleanly, port $PORT freed"

# ─────────────────────────────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────────────────────────────

echo
echo "================================================================"
echo "  US-007 verification: all 7 acceptance criteria pass."
echo "  Logs: $LOG_DIR"
echo "================================================================"
exit 0
