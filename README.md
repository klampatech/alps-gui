# alps-gui

**Dioxus 0.7 UI for [klampatech/alps](https://github.com/klampatech/alps).**

Single Rust crate (`alps-ui/`) that adds a web + desktop + mobile UI on top of the ALPS orchestrator without splitting the codebase. The UI is a thin presentational layer — it reads the on-disk artifacts (`tasks/<id>/plan.json`, `review.json`, `receipts.json`, `AGENTS.md`, etc.) and spawns `alps run` as a child process. The orchestrator's type-state machine stays the single source of truth for state transitions.

## Status

✅ **Smoke #1 complete** (8/8 user stories, 2026-08-24). Working UI visible at `127.0.0.1:5274` via `dx serve --platform server --features server`.

Source of truth for design + progress:
- [`SPEC.md`](./SPEC.md) — full design
- [`DESIGN.md`](./DESIGN.md) — visual language + components
- **Vault page:** `~/Obsidian/projects/alps-ui-spec.md`
- **Smoke-A2 roadmap:** `~/Obsidian/projects/alps-ui-smoke-A2-brief.md`

## Architecture in one sentence

A Dioxus 0.7 fullstack app (`alps-ui/`) that wraps the ALPS orchestrator's CLI — `alps list --json` for the task list, `alps show --json` for task detail, `alps run --prompt-file` to spawn new work. The UI is presentational; the orchestrator owns state.

## Repository layout

```
klampatech/alps-gui/                     # THIS repo
├── Cargo.toml                           # own Cargo workspace
├── alps-ui/                             # the GUI crate
│   ├── Cargo.toml
│   ├── Dioxus.toml
│   ├── assets/                          # main.css, tailwind.css, favicon
│   ├── public/                          # (created by dx serve bundle; gitignored)
│   ├── src/
│   │   ├── main.rs                      # App + Router + Stylesheet
│   │   ├── routes.rs                    # typed Route enum (7 variants)
│   │   ├── domain.rs                    # re-exports from alps_core + UI TaskId
│   │   ├── fixtures.rs                  # (smoke #1 only; removed in M1)
│   │   ├── layouts/nav.rs               # NavBar layout
│   │   ├── pages/                       # Dashboard, NewTask, TaskDetail, TaskLog, TaskDiff, Settings, NotFound
│   │   ├── components/                  # StatusPill, StoryCard, FindingCard, AssertionCard, ReceiptCard, ResponsiveGrid
│   │   └── api/                         # #[server] functions behind #[cfg(feature = "server")]
│   └── tailwind.config.js
├── scripts/
│   ├── alps-gui-smoke-A.sh              # smoke #1 wrapper (historical)
│   └── verify-us-007.sh                 # US-007 acceptance suite (7 checks; CSS-load check #4b added 2026-08-24)
├── SPEC.md
├── DESIGN.md
└── README.md
```

## Development

### Build

```bash
cargo build --bin alps-ui                              # default (web)
cargo build --bin alps-ui --features fullstack         # fullstack (wasm32 client + axum server)
cargo build --bin alps-ui --features server            # SSR mode (no wasm32 required)
```

### Serve

```bash
# SSR mode — fastest to iterate, renders fixtures (smoke #1) or live data (smoke-A2 M1+)
dx serve --port 5274 --platform server --package alps-ui --features server

# WASM mode — production-like, requires wasm32-unknown-unknown target
dx serve --port 5174
```

### Verify

```bash
# Runs all 7 acceptance checks: 3 cargo builds + clippy + dx serve + 2 HTML probes + CSS-load
bash scripts/verify-us-007.sh --port 5274
```

## Contributing

**`main` is protected — all changes go via PR.** Use `gh pr create` from a feature branch; CI must pass before merge. To set up your first PR:

```bash
git checkout -b feat/<short-name>
# ... make changes, commit ...
git push origin feat/<short-name>
gh pr create --base main --head feat/<short-name>
```

CI (`.github/workflows/ci.yaml`) runs:
- `cargo build --bin alps-ui` (default web)
- `cargo build --bin alps-ui --features fullstack`
- `cargo build --bin alps-ui --features server`
- `cargo test --bin alps-ui`
- `cargo clippy --bin alps-ui --no-deps -- -D warnings`
- `bash scripts/verify-us-007.sh --port 5274`

The verify script exercises the actual UI serve — it spins up `dx serve`, curls the Dashboard, parses the SSR'd HTML for state labels, fetches every `<link rel="stylesheet">` URL (catches the unstyled-HTML defect class that slipped through smoke #1).

## Next step

**Smoke-A2** (M1: live Dashboard via `use_resource` reading `alps list --json`). Roadmap in `~/Obsidian/projects/alps-ui-smoke-A2-brief.md`.
