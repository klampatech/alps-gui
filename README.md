# alps-gui

**Dioxus 0.7 UI for [klampatech/alps](https://github.com/klampatech/alps).**

Single Rust crate (`alps-ui/`, planned) that adds a web + desktop + mobile UI on top of the ALPS orchestrator without splitting the codebase. The UI is a thin presentational layer — it reads the on-disk artifacts (`tasks/<id>/plan.json`, `review.json`, `receipts.json`, `AGENTS.md`, etc.) and spawns `alps run` as a child process. The orchestrator's type-state machine stays the single source of truth for state transitions.

## Status

🚧 **Smoke A (Topic A + Topic E first cut) planned.** No code yet. The design and acceptance criteria live in:

- [`SPEC.md`](./SPEC.md) — full design (~58 KB, 738+ lines, updated 2026-08-23 with §4.2 workspace-layout decision)
- [`DESIGN.md`](./DESIGN.md) — visual language, components, layout rules (~12 KB)
- **Source of truth (vault):** `~/Obsidian/projects/alps-ui-spec.md` (mirrors SPEC.md)

The orchestrator lives at [`klampatech/alps`](https://github.com/klampatech/alps) — separate repo, separate Cargo workspace. `alps-ui` depends on `alps-core` via `path = "../alps/alps-core"`. Both repos sit side-by-side on disk.

## Architecture in one sentence

A Dioxus 0.7 fullstack app (`alps-ui/`) that wraps the ALPS orchestrator's CLI — `alps list --json` for the task list, `alps show --json` for task detail, `alps run --prompt-file` to spawn new work. The UI is presentational; the orchestrator owns state.

## Repository layout

```
klampatech/alps-gui/                     # THIS repo
├── Cargo.toml                           # own Cargo workspace
├── Dioxus.toml                          # Dioxus 0.7 config
├── SPEC.md                              # full design (mirrors vault)
├── DESIGN.md                            # visual + component rules
├── README.md                            # you are here
├── assets/                              # static assets (Tailwind, favicon)
└── alps-ui/                             # the GUI crate (planned)
    ├── Cargo.toml
    ├── src/
    └── tests/
```

## Next step

**Smoke A** — Topic A (workspace + scaffold + `alps-core` path dep) and the first cut of Topic E (Dashboard page with fixtures, no SSE). The orchestrator runs this smoke against this repo via:

```bash
PROMPT_FILE=$(mktemp -t alps-prompt.XXXXXX.txt)
# ... write smoke prompt to $PROMPT_FILE ...
mkdir -p ~/Development/alps-runs/alps-gui-smoke-A
alps run \
    --workdir ~/Development/alps-runs/alps-gui-smoke-A \
    --deliverable-path ~/Development/alps-gui \
    --prompt-file "$PROMPT_FILE"
```

See `SPEC.md` §12 for the full Topic A–G implementation milestones. Smoke A covers A + first part of E.
