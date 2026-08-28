#!/usr/bin/env bash
# alps-gui snapshot fixture workdir builder — M5 (PR #11).
#
# Builds /tmp/alps-ui-snapshot-fixture/ with deterministic content so the
# visual snapshot test renders consistent UI regardless of the host's
# real ~/Development/alps-runs state. The fixture has ONE task in
# `planned` state with a Plan attached — enough to exercise the
# Dashboard's TaskCard + the TaskDetail page's plan rendering.
#
# ## Why a fixture (not the real workdir)
#
# The real-workdir approach was tried first but hit the M4-proper
# "Settings initial-load race" — the Dashboard's `use_resource(tasks_list)`
# fires with the wasm fallback workdir (~/.alps-runs on Linux), not the
# post-App-mount get_workdir value. Until that's fixed, snapshots against
# the real workdir render the empty-state. A hermetic fixture sidesteps
# the race and makes CI deterministic.
#
# ## Why ONE task (not multiple states)
#
# The visual snapshot suite's job is "catch CSS regressions" — the
# rendering of a populated Dashboard. Whether there are 1 task or 18
# tasks doesn't matter for CSS. One task is enough to prove "cards
# render." If you want state-coverage, that's a separate test (the
# unit tests in `pages/dashboard.rs` already cover StatusPill colors
# for all 9 TaskState variants).
#
# ## Layout
#
#   /tmp/alps-ui-snapshot-fixture/
#     tasks/
#       2025-01-01T000000-fixturesnap01/
#         prompt.md
#         plan.json
#         implementation.json   (empty-state for the implementation card)
#     .alps-telemetry.log       (empty)
#
# ## Idempotency
#
# Re-running overwrites /tmp/alps-ui-snapshot-fixture/ in place. The
# script does NOT touch the user's real workdir or any git state.

set -euo pipefail

FIXTURE_ROOT="${ALPS_UI_SNAPSHOT_FIXTURE_ROOT:-/tmp/alps-ui-snapshot-fixture}"
TASK_ID="2025-01-01T000000-fixturesnap01"
TASK_DIR="$FIXTURE_ROOT/tasks/$TASK_ID"

echo "Building fixture workdir at $FIXTURE_ROOT..."

# Clean slate
rm -rf "$FIXTURE_ROOT"
mkdir -p "$TASK_DIR"

# prompt.md
cat > "$TASK_DIR/prompt.md" <<'PROMPT'
Snapshot fixture prompt: add a placeholder Dashboard task card so the visual snapshot test has a non-empty Tasks list to render.
PROMPT

# plan.json — single story so the TaskDetail Plan section renders one StoryCard
cat > "$TASK_DIR/plan.json" <<'PLAN'
{
  "id": "2025-01-01T000000-fixtureplan01",
  "goal": "Render a non-empty Dashboard so the M5 visual snapshot suite has content to capture.",
  "architecture": "Single fixture task in /tmp/alps-ui-snapshot-fixture/. Loaded by tests/responsive_layout.rs via ALPS_UI_WORKDIR. No orchestration runs — the fixture is read-only on-disk content.",
  "stories": [
    {
      "id": "2025-01-01T000000-fixturestory01",
      "title": "Render one task card",
      "description": "The Dashboard's TaskCard component must render at least one card when the snapshot test loads /, so the baseline is not the empty-state placeholder.",
      "acceptance_criteria": [
        "TaskCard renders the fixture task's task_id",
        "StatusPill shows the planned color",
        "Prompt excerpt is visible (truncated to 200 chars)"
      ],
      "priority": 1
    }
  ],
  "dod": [
    { "criterion": "Fixture is hermetic", "verifiable": true }
  ]
}
PLAN

# implementation.json — present so TaskDetail shows an Implementation section
cat > "$TASK_DIR/implementation.json" <<'IMPL'
{
  "ralph_branch": "snap-fixture-2025-01-01",
  "prd_path": "/tmp/alps-ui-snapshot-fixture/tasks/2025-01-01T000000-fixturesnap01/implementation/ralph/prd.md",
  "commits": [
    { "sha": "abcdef0123456789", "message": "snapshot fixture: seed Dashboard with one planned task" }
  ],
  "artifacts": [],
  "metrics": { "stories_passed": 0, "stories_total": 1, "iterations": 0, "elapsed_secs": 0 },
  "deliverable_path": "/tmp/alps-ui-snapshot-fixture"
}
IMPL

# Empty telemetry log so `tail .alps-telemetry.log` (TaskLog pane) renders
touch "$FIXTURE_ROOT/.alps-telemetry.log"

echo "Fixture ready:"
echo "  workdir: $FIXTURE_ROOT"
echo "  task_id: $TASK_ID"
ls -la "$TASK_DIR"
echo "---"
echo "verify with: alps list --json --workdir $FIXTURE_ROOT"
