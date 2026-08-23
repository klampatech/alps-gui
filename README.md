# alps-gui

**Dioxus 0.7 UI for [klampatech/alps](https://github.com/klampatech/alps).**

Single Rust crate (`alps-ui/`, planned) that adds a web + desktop + mobile UI on top of the ALPS orchestrator without splitting the codebase. The UI is a thin presentational layer — it reads the on-disk artifacts (`tasks/<id>/plan.json`, `review.json`, `receipts.json`, `AGENTS.md`, etc.) and spawns `alps run` as a child process. The orchestrator's type-state machine stays the single source of truth for state transitions.

## Status

🚧 **Spec drafted 2026-08-23.** No code yet. The design and acceptance criteria live in [`SPEC.md`](./SPEC.md).

| What | Where |
|---|---|
| Full design | [`SPEC.md`](./SPEC.md) (~53 KB, 738 lines) |
| Canonical spec (vault) | `~/Obsidian/projects/alps-ui-spec.md` |
| Upstream orchestrator | [`klampatech/alps`](https://github.com/klampatech/alps) |
| Dioxus reference | [dioxuslabs.com/learn/0.7](https://dioxuslabs.com/learn/0.7/) |

## Architecture in one sentence

A Dioxus 0.7 fullstack crate (`alps-ui/`) added to the existing `klampatech/alps` Cargo workspace, sharing `alps-core` types directly, with one Rust binary that ships WASM (web), a WebView (desktop), or a native shell (mobile) from the same `rsx!{}` tree.

## Next step

Topic A of the spec — add `alps-ui/` to the workspace + scaffold via `dx new` + wire `alps-core` as a dependency. Each topic is a 1-session coding task; see [`SPEC.md`](./SPEC.md) §12.
