#!/usr/bin/env bash
# ALPS-on-ALPS-GUI smoke wrapper — smoke A.
#
# Pattern after `/tmp/alps-tier4-smoke-wrapper.sh` (SPEC §9.8 / §12
# item 9.8). Parameterized wrapper: same diagnostic machinery across
# runs, only the values change.
#
# What this smoke does:
#   1. Creates a throwaway workdir (~/Development/alps-runs/<label>)
#   2. Writes the smoke prompt to a mktemp file (~50 chars in argv)
#   3. Spawns `alps run --workdir <workdir> --deliverable-path
#      ~/Development/alps-gui --prompt-file <file>` via the canonical
#      argv-leak-safe shape (SPEC §12 item 9.7)
#   4. Tails the orchestrator stderr + telemetry to logs/ for
#      post-mortem analysis
#
# Usage:
#   ./scripts/alps-gui-smoke-A.sh [--label <name>] [--dry-run]
#
# Environment overrides (any of these):
#   ALPS_GUI_SMOKE_LABEL            default: alps-gui-smoke-A
#   ALPS_GUI_SMOKE_WORKDIR_PARENT   default: ~/Development/alps-runs
#   ALPS_GUI_SMOKE_DELIVERABLE_PATH default: ~/Development/alps-gui
#   ALPS_GUI_SMOKE_PROMPT_FILE      default: ./scripts/prompts/smoke-A.txt
#   ALPS_GUI_SMOKE_LOG_PREFIX       default: /tmp/alps-gui-smoke-A

set -euo pipefail

# ─────────────────────────────────────────────────────────────────────
# Parameters
# ─────────────────────────────────────────────────────────────────────

LABEL="${ALPS_GUI_SMOKE_LABEL:-alps-gui-smoke-A}"
WORKDIR_PARENT="${ALPS_GUI_SMOKE_WORKDIR_PARENT:-$HOME/Development/alps-runs}"
DELIVERABLE_PATH="${ALPS_GUI_SMOKE_DELIVERABLE_PATH:-$HOME/Development/alps-gui}"
# The prompt file MUST live OUTSIDE the deliverable path. The LLM
# running inside the smoke has workspace-write access to the
# deliverable path, so an in-repo copy at scripts/prompts/smoke-A.txt
# is at risk of being deleted by the LLM as part of its scaffolding
# work (verified 2026-08-23: codex deleted it during the smoke).
# The out-of-band copy at ~/.local/share/alps/smoke-prompts/smoke-A.txt
# is the canonical one. The in-repo copy is fallback only.
PROMPT_FILE="${ALPS_GUI_SMOKE_PROMPT_FILE:-$HOME/.local/share/alps/smoke-prompts/smoke-A.txt}"
LOG_PREFIX="${ALPS_GUI_SMOKE_LOG_PREFIX:-/tmp/alps-gui-smoke-A}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run) DRY_RUN=true; shift ;;
        --label) LABEL="$2"; shift 2 ;;
        --workdir-parent) WORKDIR_PARENT="$2"; shift 2 ;;
        --deliverable-path) DELIVERABLE_PATH="$2"; shift 2 ;;
        --prompt-file) PROMPT_FILE="$2"; shift 2 ;;
        --log-prefix) LOG_PREFIX="$2"; shift 2 ;;
        -h|--help)
            sed -n '2,/^$/p' "$0" | sed 's/^# \?//'
            exit 0
            ;;
        *)
            echo "error: unknown argument: $1" >&2
            exit 2
            ;;
    esac
done

WORKDIR="$WORKDIR_PARENT/$LABEL"
STDERR_LOG="${LOG_PREFIX}-stderr.log"
TELEMETRY_LOG="${LOG_PREFIX}-telemetry.log"
SIGTERM_LOG="${LOG_PREFIX}-sigterm.log"
ALPS_BIN="$HOME/Development/alps/target/release/alps"

# Codex config quirk: ~/.codex/config.toml declares
#   env_key = "MINIMAX_API_KEY_UNUSED"
# as a "dummy placeholder that codex requires to be non-empty."
# Codex's runtime validates the env var *exists* before sending
# a request, even though the local key-proxy at :8789 injects the
# real key from its own EnvironmentFile. Without this, every codex
# invocation fails with:
#   ERROR: Missing environment variable: MINIMAX_API_KEY_UNUSED.
# The value doesn't matter — any non-empty string works.
export MINIMAX_API_KEY_UNUSED="${MINIMAX_API_KEY_UNUSED:-placeholder-for-codex-config-validation}"

# ─────────────────────────────────────────────────────────────────────
# Pre-flight
# ─────────────────────────────────────────────────────────────────────

if [ -d "$WORKDIR" ]; then
    echo "error: workdir $WORKDIR already exists; refusing to clobber" >&2
    echo "  → if you really want to re-run, delete it first" >&2
    exit 2
fi

if [ "${DRY_RUN:-false}" = true ]; then
    echo "[alps-gui-smoke] (dry-run mode — skipping cargo build + alps launch)"
    [ ! -x "$ALPS_BIN" ] && echo "  → $ALPS_BIN not found; cargo build --workspace --release needed before real run" >&2
    [ ! -f "$PROMPT_FILE" ] && echo "  → prompt file not found at $PROMPT_FILE" >&2
    [ ! -d "$DELIVERABLE_PATH" ] && echo "  → deliverable path $DELIVERABLE_PATH does not exist" >&2
    EXPECTED_BRANCH="${ALPS_GUI_SMOKE_EXPECTED_BRANCH:-feat/alps-gui-prereq}"
    actual_branch=$(cd "$HOME/Development/alps" && git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown")
    if [ "$actual_branch" != "$EXPECTED_BRANCH" ]; then
        echo "  → alps repo on '$actual_branch', expected '$EXPECTED_BRANCH' (the pre-req branch)" >&2
    else
        local_sha=$(cd "$HOME/Development/alps" && git rev-parse HEAD 2>/dev/null)
        echo "  → alps branch OK: $actual_branch @ $local_sha"
    fi
    echo "[alps-gui-smoke] ✓ preflight OK (dry-run; not launching alps)"
    exit 0
fi

# Branch guard — refuse to build from main or any non-feature branch.
# The pre-req (alps list / alps show JSON contract) lives on a feature
# branch; building from main would silently miss it. Override with
# ALPS_GUI_SMOKE_SKIP_BRANCH_CHECK=1 if you really mean to.
EXPECTED_BRANCH="${ALPS_GUI_SMOKE_EXPECTED_BRANCH:-feat/alps-gui-prereq}"
actual_branch=$(cd "$HOME/Development/alps" && git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown")
if [ "${ALPS_GUI_SMOKE_SKIP_BRANCH_CHECK:-0}" != "1" ]; then
    if [ "$actual_branch" != "$EXPECTED_BRANCH" ]; then
        echo "error: alps repo is on branch '$actual_branch', expected '$EXPECTED_BRANCH'" >&2
        echo "  → the alps-gui pre-req (alps list / alps show --json) lives on $EXPECTED_BRANCH" >&2
        echo "  → to switch:  cd ~/Development/alps && git checkout $EXPECTED_BRANCH" >&2
        echo "  → to override: ALPS_GUI_SMOKE_SKIP_BRANCH_CHECK=1 $0" >&2
        exit 2
    fi
    # Also assert HEAD matches the remote tip — catches "I forgot to push".
    local_sha=$(cd "$HOME/Development/alps" && git rev-parse HEAD 2>/dev/null || echo "unknown")
    remote_sha=$(cd "$HOME/Development/alps" && git rev-parse origin/$EXPECTED_BRANCH 2>/dev/null || echo "unknown")
    if [ "$local_sha" != "$remote_sha" ] && [ "$remote_sha" != "unknown" ]; then
        echo "warning: local HEAD ($local_sha) does not match origin/$EXPECTED_BRANCH ($remote_sha)" >&2
        echo "  → the smoke will build from local HEAD; push first if you want a stable reference" >&2
    fi
fi

# Build the orchestrator first so a smoke can't begin with a stale binary.
echo "[alps-gui-smoke] building alps CLI..."
(cd "$HOME/Development/alps" && cargo build --workspace --release) >/dev/null

if [ ! -x "$ALPS_BIN" ]; then
    echo "error: $ALPS_BIN not found after cargo build" >&2
    echo "  → cargo test --workspace --no-run does NOT produce the binary" >&2
    echo "  → always run cargo build --workspace before a smoke" >&2
    exit 2
fi

if [ ! -f "$PROMPT_FILE" ]; then
    echo "error: prompt file not found at $PROMPT_FILE" >&2
    echo "  → restore from in-repo copy: cp $HOME/Development/alps-gui/scripts/prompts/smoke-A.txt $PROMPT_FILE" >&2
    exit 2
fi

if [ ! -d "$DELIVERABLE_PATH" ]; then
    echo "error: deliverable path $DELIVERABLE_PATH does not exist" >&2
    echo "  → did you forget to git init + commit the SPEC.md / DESIGN.md?" >&2
    exit 2
fi

mkdir -p "$WORKDIR"
: > "$STDERR_LOG"
: > "$TELEMETRY_LOG"
: > "$SIGTERM_LOG"

echo "[alps-gui-smoke] workdir:           $WORKDIR"
echo "[alps-gui-smoke] deliverable:       $DELIVERABLE_PATH"
echo "[alps-gui-smoke] prompt file:       $PROMPT_FILE"
echo "[alps-gui-smoke] stderr log:        $STDERR_LOG"
echo "[alps-gui-smoke] telemetry log:     $TELEMETRY_LOG"
echo "[alps-gui-smoke] sigterm log:       $SIGTERM_LOG"
echo "[alps-gui-smoke] alps binary:       $ALPS_BIN"
echo "[alps-gui-smoke] alps branch:       $actual_branch @ ${local_sha:-unknown}"

# ─────────────────────────────────────────────────────────────────────
# Run
# ─────────────────────────────────────────────────────────────────────

cd "$WORKDIR"

echo "[alps-gui-smoke] launching alps run..."

"$ALPS_BIN" run \
    --workdir "$WORKDIR" \
    --deliverable-path "$DELIVERABLE_PATH" \
    --prompt-file "$PROMPT_FILE" \
    --telemetry-log "$TELEMETRY_LOG" \
    >"$STDERR_LOG" 2>&1

exit_code=$?
echo "[alps-gui-smoke] alps exited with code $exit_code"

# ─────────────────────────────────────────────────────────────────────
# Post-mortem summary
# ─────────────────────────────────────────────────────────────────────

if [ -f "$HOME/Development/alps-gui/alps-ui/Cargo.toml" ]; then
    echo "[alps-gui-smoke] ✓ alps-ui/Cargo.toml exists"
else
    echo "[alps-gui-smoke] � alps-ui/Cargo.toml MISSING"
fi

if [ -f "$HOME/Development/alps-gui/alps-ui/src/main.rs" ]; then
    echo "[alps-gui-smoke] ✓ alps-ui/src/main.rs exists"
else
    echo "[alps-gui-smoke] ✗ alps-ui/src/main.rs MISSING"
fi

if [ -f "$SIGTERM_LOG" ]; then
    sig_count=$(grep -c "received" "$SIGTERM_LOG" 2>/dev/null || echo 0)
    echo "[alps-gui-smoke] signal handler activations: $sig_count"
fi

if [ -d "$WORKDIR/tasks" ]; then
    task_count=$(ls -1 "$WORKDIR/tasks" 2>/dev/null | wc -l)
    echo "[alps-gui-smoke] tasks created: $task_count"
    if [ "$task_count" -gt 0 ]; then
        latest_task=$(ls -1t "$WORKDIR/tasks" | head -1)
        echo "[alps-gui-smoke] latest task: $latest_task"
        if [ -f "$WORKDIR/tasks/$latest_task/receipts.json" ]; then
            verdict=$(python3 -c "import json; print(json.load(open('$WORKDIR/tasks/$latest_task/receipts.json'))['judge_model'])" 2>/dev/null || echo "?")
            echo "[alps-gui-smoke] ✓ Judge ACCEPT — judge_model=$verdict"
        elif [ -f "$WORKDIR/tasks/$latest_task/feedback.json" ]; then
            echo "[alps-gui-smoke] ✗ Judge REJECT — see $WORKDIR/tasks/$latest_task/feedback.json"
        elif [ -f "$WORKDIR/tasks/$latest_task/failure.json" ]; then
            echo "[alps-gui-smoke] ✗ CATASTROPHIC FAILURE — see $WORKDIR/tasks/$latest_task/failure.json"
        else
            echo "[alps-gui-smoke] ? task $latest_task still in flight or incomplete"
        fi
    fi
fi

echo "[alps-gui-smoke] stderr: $STDERR_LOG ($(wc -l < "$STDERR_LOG") lines)"
echo "[alps-gui-smoke] telemetry: $TELEMETRY_LOG ($(wc -l < "$TELEMETRY_LOG") lines)"

exit $exit_code
