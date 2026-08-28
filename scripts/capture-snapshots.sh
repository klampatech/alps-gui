#!/usr/bin/env bash
# alps-gui visual snapshot capture — M5 (PR #11).
#
# Captures 7 routes × 3 viewports = 21 PNG baselines into
# tests/snapshots/<viewport>/<route>.png using headless Chromium.
# Used for both:
#   - Initial baseline creation (commit the PNGs as the visual contract)
#   - UPDATE_SNAPSHOTS=1 refresh after intentional UI changes
#   - CI gate verification (cargo test --test responsive_layout runs the same
#     capture, then compares against the committed baselines)
#
# Recipe follows M4-prep's Pitfall 52: --virtual-time-budget=8000 lets
# Dioxus hydration complete before the snapshot (otherwise the screenshot
# captures the loading skeleton instead of the populated card).
#
# Usage:
#   bash scripts/capture-snapshots.sh [--port <PORT>] [--workdir <DIR>]
#                                     [--chromium <PATH>] [--route <route>]
#
# Defaults:
#   --port      5361 (avoids overlap with the verify-script's 5274)
#   --workdir   $HOME/Development/alps-runs (real workdir — see note below)
#   --chromium  /usr/bin/chromium (Debian's `chromium` package; falls back
#               to ~/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome)
#
# Routes captured: /, /tasks/new, /tasks/<SAMPLE_ID>, /tasks/<SAMPLE_ID>/log,
#                  /tasks/<SAMPLE_ID>/diff, /settings, /not-found
# Viewports: 375, 768, 1280
#
# ## Why real workdir, not a fixture
#
# The snapshot baselines are committed alongside the code that renders them.
# They capture "what the UI looks like against a real workdir at the moment
# this baseline was committed." When the workdir changes (new task spawns,
# existing task's state advances), the baselines will diff. That's
# intentional — refresh with `UPDATE_SNAPSHOTS=1 bash scripts/capture-snapshots.sh`
# to regenerate. The CONTRIBUTING section of README.md documents this.
#
# We considered a fixture workdir (hand-written task JSONs in a /tmp tree)
# but the maintenance burden (200+ lines of JSON, every TaskSummary field
# to keep in sync with alps-core's schema) isn't worth the stability gain.
# Real workdir + UPDATE_SNAPSHOTS=1 is the standard Playwright/Jest pattern.

set -euo pipefail

# ─────────────────────────────────────────────────────────────────────
# Parameters
# ─────────────────────────────────────────────────────────────────────

PORT="${ALPS_UI_SNAPSHOT_PORT:-5361}"
WORKDIR="${ALPS_UI_SNAPSHOT_WORKDIR:-$HOME/Development/alps-runs}"
CHROMIUM="${ALPS_UI_SNAPSHOT_CHROMIUM:-}"
ONLY_ROUTE=""
VIRTUAL_TIME_BUDGET="${ALPS_UI_VIRTUAL_TIME_BUDGET:-8000}"  # ms, per Pitfall 52

VIEWPORTS=(375 768 1280)
VIEWPORT_HEIGHTS=(667 1024 800)  # portrait-ish heights matching common devices
ROUTES=(
    "/"
    "/tasks/new"
    "/tasks/__SAMPLE_ID__"
    "/tasks/__SAMPLE_ID__/log"
    "/tasks/__SAMPLE_ID__/diff"
    "/settings"
    "/__NOT_FOUND__"
)

# Replace placeholders with real values from the workdir after we know the
# task_id we'll capture against (filled in later in main()).

# ─────────────────────────────────────────────────────────────────────
# Argument parsing
# ─────────────────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --port) PORT="$2"; shift 2;;
        --workdir) WORKDIR="$2"; shift 2;;
        --chromium) CHROMIUM="$2"; shift 2;;
        --route) ONLY_ROUTE="$2"; shift 2;;
        -h|--help)
            grep -E '^#( |$)' "$0" | sed 's/^# \?//'
            exit 0;;
        *) echo "Unknown arg: $1" >&2; exit 2;;
    esac
done

# ─────────────────────────────────────────────────────────────────────
# Chromium discovery
# ─────────────────────────────────────────────────────────────────────

if [[ -z "$CHROMIUM" ]]; then
    for candidate in \
        /usr/bin/chromium \
        /usr/bin/chromium-browser \
        /usr/bin/google-chrome \
        "$HOME/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome" \
        "$HOME/.cache/ms-playwright/chromium-1223/chrome-linux64/chrome"; do
        if [[ -x "$candidate" ]]; then
            CHROMIUM="$candidate"
            break
        fi
    done
fi

if [[ -z "$CHROMIUM" ]]; then
    echo "ERROR: no chromium found. Install chromium-browser or set ALPS_UI_SNAPSHOT_CHROMIUM." >&2
    exit 3
fi

echo "Using chromium: $CHROMIUM"
"$CHROMIUM" --version 2>&1 | head -1 || true

# ─────────────────────────────────────────────────────────────────────
# Resolve sample task_id from the workdir (first task in alps list)
# ─────────────────────────────────────────────────────────────────────

SAMPLE_TASK_ID="$(alps list --json --workdir "$WORKDIR" 2>/dev/null \
    | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["tasks"][0]["task_id"] if d.get("tasks") else "")')"

if [[ -z "$SAMPLE_TASK_ID" ]]; then
    echo "ERROR: workdir '$WORKDIR' has no tasks. Cannot resolve a sample task_id." >&2
    echo "       Either run a task via 'alps run' first, or pass --workdir <other>." >&2
    exit 4
fi

echo "Using sample task_id: $SAMPLE_TASK_ID"

# Materialize routes with sample_id substituted in
RESOLVED_ROUTES=()
for r in "${ROUTES[@]}"; do
    r="${r/__SAMPLE_ID__/$SAMPLE_TASK_ID}"
    r="${r/__NOT_FOUND__/_does_not_exist}"
    RESOLVED_ROUTES+=("$r")
done

# Filter if --route was given (substring match)
if [[ -n "$ONLY_ROUTE" ]]; then
    FILTERED=()
    for r in "${RESOLVED_ROUTES[@]}"; do
        if [[ "$r" == *"$ONLY_ROUTE"* ]]; then
            FILTERED+=("$r")
        fi
    done
    RESOLVED_ROUTES=("${FILTERED[@]}")
    if [[ ${#RESOLVED_ROUTES[@]} -eq 0 ]]; then
        echo "ERROR: --route '$ONLY_ROUTE' matched no routes." >&2
        exit 5
    fi
fi

# ─────────────────────────────────────────────────────────────────────
# Output directory
# ─────────────────────────────────────────────────────────────────────

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SNAPSHOT_DIR="$REPO_ROOT/alps-ui/tests/snapshots"
mkdir -p "$SNAPSHOT_DIR"

# ─────────────────────────────────────────────────────────────────────
# dx serve lifecycle
# ─────────────────────────────────────────────────────────────────────

SERVE_PID=""
cleanup_serve() {
    if [[ -n "$SERVE_PID" ]] && kill -0 "$SERVE_PID" 2>/dev/null; then
        echo "Stopping dx serve (pid $SERVE_PID)..."
        kill "$SERVE_PID" 2>/dev/null || true
        sleep 1
        kill -9 "$SERVE_PID" 2>/dev/null || true
    fi
    # Also kill any orphaned dx serve on this port
    pkill -f "dx serve.*--port $PORT" 2>/dev/null || true
}
trap cleanup_serve EXIT

echo "Starting dx serve on port $PORT with ALPS_UI_WORKDIR=$WORKDIR..."
ALPS_UI_WORKDIR="$WORKDIR" dx serve \
    --port "$PORT" \
    --platform server \
    --features server \
    --package alps-ui \
    > /tmp/alps-ui-snapshot-serve.log 2>&1 &
SERVE_PID=$!

# Wait for the server to bind (Pitfall-style retry, max 60s)
echo "Waiting for dx serve to bind 127.0.0.1:$PORT..."
for i in $(seq 1 60); do
    if curl -s -o /dev/null -w "%{http_code}" "http://127.0.0.1:$PORT/" 2>/dev/null | grep -q '^200$'; then
        echo "  bound after ${i}s"
        break
    fi
    sleep 1
    if [[ $i -eq 60 ]]; then
        echo "ERROR: dx serve did not bind within 60s. Tail of log:" >&2
        tail -40 /tmp/alps-ui-snapshot-serve.log >&2
        exit 6
    fi
done

# ─────────────────────────────────────────────────────────────────────
# Capture loop
# ─────────────────────────────────────────────────────────────────────

TOTAL=${#RESOLVED_ROUTES[@]}
CAPTURED=0
for viewport in "${VIEWPORTS[@]}"; do
    # Map viewport to its height
    case "$viewport" in
        375) HEIGHT=667;;
        768) HEIGHT=1024;;
        1280) HEIGHT=800;;
        *) HEIGHT=800;;
    esac

    mkdir -p "$SNAPSHOT_DIR/$viewport"

    for route in "${RESOLVED_ROUTES[@]}"; do
        SAFE_NAME="$(echo "$route" | sed 's|/|_|g; s|^_||; s|^$|root|')"
        OUT="$SNAPSHOT_DIR/$viewport/${SAFE_NAME}.png"
        URL="http://127.0.0.1:$PORT$route"

        echo "  capture $viewport/${SAFE_NAME}.png  ($URL)"

        # Use --hide-scrollbars + --virtual-time-budget for stable snapshots.
        # --no-sandbox for CI containers (root user); safe locally too.
        # --disable-gpu because software rendering is fine for static captures.
        "$CHROMIUM" \
            --headless=new \
            --disable-gpu \
            --no-sandbox \
            --hide-scrollbars \
            --window-size="${viewport},${HEIGHT}" \
            --virtual-time-budget="$VIRTUAL_TIME_BUDGET" \
            --screenshot="$OUT" \
            "$URL" \
            > /tmp/alps-ui-snapshot-${viewport}-${SAFE_NAME}.log 2>&1 \
            || { echo "ERROR: chromium failed for $URL. Log:" >&2; tail -10 /tmp/alps-ui-snapshot-${viewport}-${SAFE_NAME}.log >&2; exit 7; }

        if [[ ! -s "$OUT" ]]; then
            echo "ERROR: $OUT is empty or missing." >&2
            tail -10 /tmp/alps-ui-snapshot-${viewport}-${SAFE_NAME}.log >&2
            exit 8
        fi

        CAPTURED=$((CAPTURED + 1))
    done
done

echo
echo "Captured $CAPTURED / $TOTAL screenshots into $SNAPSHOT_DIR"
echo "Tree:"
find "$SNAPSHOT_DIR" -type f -name '*.png' | sort | sed "s|^$REPO_ROOT/|  |"
