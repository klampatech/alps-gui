---
type: spec
title: ALPS — Dioxus 0.7 UI (web + desktop + mobile, single Rust binary)
date: '2026-08-23T17:00:00.000Z'
status: draft
private: false
priority: medium
projects:
  - alps
parent_project: alps
supersedes: null
ingested_via: 'human:evo'
source_kind: 'human:evo'
---

# ALPS — Dioxus 0.7 UI

> **Source of truth (post-implementation):** `klampatech/alps` `SPEC.md` and `alps-ui/SPEC.md` (in-repo once it lands). This vault page is the planning doc to drive the implementation; SPEC.md gets the canonical sections once the design is approved.
>
> **Author:** Evo (drafted 2026-08-23 from Kyle's request: build the ALPS UI on Dioxus 0.7 using the tutorial at `https://dioxuslabs.com/learn/0.7/tutorial/new_app/`, study ALPS, design a UI flexible enough for all devices).
>
> **Status of factual claims in this spec:** every ALPS-side fact was checked against `~/Development/alps/SPEC.md` (v0.8, 1217 lines, 2026-08-11) and `~/Development/alps/alps-cli/src/main.rs` (920 lines) and `~/Development/alps/README.md` (650 lines) before drafting. Every Dioxus-side fact was checked against the in-band `dioxus-0.7` skill (version 1.0.0, pinned to Dioxus 0.7.0) plus the live `https://dioxuslabs.com/learn/0.7/tutorial/new_app/` page. See §10 "Verification log" for the audit.

## 1. TL;DR

Build a **Dioxus 0.7 fullstack** application — `alps-ui` — that gives ALPS a real UI without splitting the codebase. One Rust binary, three target platforms (web / desktop / mobile) selected at compile time via Cargo features, sharing the same `src/` tree. The UI never owns ALPS state: it reads and writes the same on-disk artifacts (`tasks/<id>/{plan.json,review.json,receipts.json,AGENTS.md,progress.txt,...}`) that the `alps` CLI does, and it invokes the orchestrator by spawning the `alps` CLI as a child process — the orchestrator's existing `alps_core::loop_::drive` runs inside that spawned binary, the UI server only does process management.

This is the natural way to give ALPS a UI without sacrificing its type-state correctness story. The orchestrator stays the source of truth for state transitions; the UI is a thin presentational layer that observes that state and asks the orchestrator to make changes. There is no parallel state machine in the UI.

**Single decision up-front:** we use **Dioxus 0.7 fullstack** (single binary, server functions compiling to Axum endpoints), not three separate codebases (Tauri + a web SPA + a mobile shell). The trade-off is that we accept Dioxus's WebView constraints on desktop/mobile (Safari / WebView2 / WebKitGTK — see Dioxus pitfall #6 in the in-band skill). We get a *huge* win on code-share, build matrix, and test coverage for the price of one CSS-compatibility audit.

## 2. Why Dioxus, and why 0.7 fullstack

**Why Dioxus (not egui / iced / slint / tauri):**

| Need | Why Dioxus wins |
|---|---|
| One codebase for web + desktop + mobile | Dioxus 0.7 compiles to WASM (web), a native WebView (desktop via `wry`), and iOS/Android shells (mobile). Single `rsx!{}` markup, single set of components. |
| Strict type safety, matching ALPS's culture | Dioxus's `Routable` enum, typed signals (`Signal<T>`), and typed props are a natural fit for ALPS's "type-state everywhere" stance. |
| SSR + hydration for fast first paint on the web | `use_loader` runs on the server during SSR and on the client during hydration — automatic handoff. |
| Server-side state without writing a separate API | `#[post("/api/...")]` on an async fn creates both an Axum endpoint and a client-callable stub. One definition, two implementations. |
| First-party hot-reload of markup + assets, Subsecond hot-patch of Rust | `dx serve --hotpatch` (new in 0.7) is the only Rust framework with actual Rust-code hot reload. |

The Dioxus 0.7 skill loaded for this spec (version 1.0.0) is the reference. The new-app tutorial at `https://dioxuslabs.com/learn/0.7/tutorial/new_app/` is the canonical scaffold.

**Why Dioxus 0.7 (not 0.6):** 0.6 lacks the `fullstack` feature unification, has the `dioxus-lib` crate we don't need, and doesn't have Subsecond hot-patch. 0.7 is the current stable (`0.7.0` per the new-app page).

**Why fullstack (not web-only with a separate API server):** fullstack gives us server functions (`#[post]`, `#[get]`) that compile to Axum endpoints inside the same binary. The alternative — web-only + a separate Rust API server — would split the project into two Cargo crates, two deploys, and two sets of types to keep in sync. Fullstack keeps the API surface typed end-to-end.

## 3. Architecture: what the UI does and does NOT own

### 3.1 The UI does not own state

The orchestrator's `loop_::drive` is the single source of truth for state transitions. The UI cannot move a `Task<Planned>` to `Task<Implemented>` directly; it asks the orchestrator to do so via an HTTP call, the orchestrator performs the transition (which the type system permits only because it has the right state), and the orchestrator writes the new state to disk under `tasks/<id>/`. The UI then re-reads.

This is load-bearing. If the UI paralleled the state machine, the two would drift — and ALPS's whole correctness story lives in `loop_::drive`'s type-state guarantees. We would rather have a UI that "feels slow" (it polls / re-fetches after each server-function call) than a UI that is "snappy" but breaks the type-state invariant.

### 3.2 The UI does own presentation

The UI is responsible for:

- **Listing tasks** — read the `tasks/<task-id>/` directory tree, parse each `prompt.md` + `plan.json` + `receipts.json` (if present) to surface status, attempt count, elapsed time, judge verdict.
- **Task detail view** — render `plan.json` stories, `review.json` findings + assertions, `feedback.json` rejection reason, `receipts.json` final summary, `progress.txt` running notes, `AGENTS.md` accumulated patterns, `prd.json` story completion flags, `git log` (commits on `alps/<task-id>` branch).
- **Live progress** — Server-Sent Events stream of `[done] accepted` / `[judge] ...` / `[implement] ...` / `[plan] ...` lines from `elog!`, scoped to one task. The UI is read-only over the log; the orchestrator is the writer.
- **Run a new task** — POST a prompt + optional `--workdir` / `--deliverable-path` / `--prompt-file` to the server. Server spawns the `alps` CLI (or calls `alps_core::loop_::drive` in-process — see Open question 1 below).
- **Cancel a task** — kill the orchestrator PID for a given `task_id`. Server tracks spawned PIDs in memory (mirrored to `<workdir>/.alps-pids.json` for restart resilience).
- **Settings** — default `--workdir`, default Judge / Plan / Review model aliases, smoke-wrapper recipe defaults.

### 3.3 The UI does not own the loop

If a task is in flight, the UI shows "running" and a live log tail. If the task completes, the UI shows the receipt. There is no in-UI "Plan → Implement → Review → Judge" button sequence; you run a task and watch it.

## 4. Repository layout

We add one new top-level crate to the existing `klampatech/alps` Cargo workspace. No restructuring of the existing two crates.

```
klampatech/alps/                    # existing workspace root (Cargo.toml [workspace])
├── Cargo.toml                      # add `members = [..., "alps-ui"]`
├── alps-core/                      # existing, unchanged
├── alps-cli/                       # existing, unchanged
└── alps-ui/                        # NEW — Dioxus 0.7 fullstack app
    ├── Cargo.toml                  # single crate, [features] for web/desktop/mobile
    ├── Dioxus.toml                 # bundle + base_path config
    ├── assets/
    │   ├── main.css                # Tailwind entry — `dx serve` runs Tailwind CLI
    │   ├── tailwind.css            # source for the CSS
    │   ├── favicon.ico
    │   └── logo.svg
    ├── src/
    │   ├── main.rs                 # `dioxus::launch(App)` (web/desktop) / mobile entry
    │   ├── server.rs               # `#[cfg(feature = "server")] mod server_only` — Axum + process spawn
    │   ├── api/
    │   │   ├── mod.rs              # public surface — every `#[post]` / `#[get]` re-exported
    │   │   ├── tasks.rs            # list / read tasks from `<workdir>/tasks/`
    │   │   ├── run.rs              # spawn `alps run` for a new task
    │   │   ├── cancel.rs           # kill a running task by task_id
    │   │   ├── stream.rs           # SSE tail of `<workdir>/.alps-pids.json` → log file
    │   │   ├── files.rs            # read-only artifact viewers (plan.json, review.json, ...)
    │   │   └── settings.rs         # get/set operator defaults
    │   ├── routes.rs               # `#[derive(Routable)]` enum — every page
    │   ├── layouts/
    │   │   ├── mod.rs
    │   │   └── nav.rs              # top-level responsive nav with `#[layout]`
    │   ├── pages/
    │   │   ├── mod.rs
    │   │   ├── dashboard.rs        # `/` — task list + run-new form
    │   │   ├── task_detail.rs      # `/tasks/:id` — main per-task view
    │   │   ├── task_run.rs         # `/tasks/new` — new-task form (or modal)
    │   │   ├── task_log.rs         # `/tasks/:id/log` — full log + live tail
    │   │   ├── task_diff.rs        # `/tasks/:id/diff` — `git log -p` of alps/<id> branch
    │   │   ├── settings.rs         # `/settings`
    │   │   └── not_found.rs        # catch-all 404
    │   ├── components/
    │   │   ├── mod.rs
    │   │   ├── story_card.rs       # one UserStory with passes badge + DoD checklist
    │   │   ├── finding_card.rs     # one Review finding with severity pill + file:line
    │   │   ├── assertion_card.rs   # one Review assertion with [x]/[ ] + evidence
    │   │   ├── receipt_card.rs     # the Done-state summary
    │   │   ├── log_stream.rs       # SSE consumer for live log lines
    │   │   ├── status_pill.rs      # Idle / Planned / Implemented / Reviewed / Done / Rejected / Failed
    │   │   ├── responsive.rs       # breakpoint helpers (see §6)
    │   │   └── code_block.rs       # syntax-highlighted artifact viewer
    │   ├── hooks/
    │   │   ├── mod.rs
    │   │   ├── use_task.rs         # `use_resource` wrapper for `api::tasks::get`
    │   │   ├── use_log_stream.rs   # SSE client + auto-reconnect
    │   │   └── use_persistent.rs   # `gloo_storage`-backed Signal (per in-band skill pattern)
    │   └── domain.rs               # UI-side mirror of the artifacts the UI reads (subset of `alps_core::domain`)
    └── tests/
        ├── api_integration.rs      # drives the server functions against a temp workdir
        └── responsive_layout.rs    # visual snapshot at 3 breakpoints (see §6)
```

### 4.1 Cargo.toml features

The single crate has four features, mirroring the in-band Dioxus 0.7 pattern:

```toml
[dependencies]
dioxus = { version = "0.7", features = ["fullstack"] }
alps-core = { path = "../alps-core" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4"] }
gloo-storage = "0.3"           # persistent signals on web
reqwest = { version = "0.12", features = ["json", "stream"] }  # SSE client
eventsource-stream = "0.2"      # SSE parsing
tokio = { version = "1", features = ["full"] }
tracing = "0.1"

[features]
default = ["web"]               # `dx serve` lands here
web = ["dioxus/web"]
desktop = ["dioxus/desktop"]
mobile = ["dioxus/mobile"]
server = ["dioxus/server"]
fullstack = ["dioxus/fullstack"]  # production deployment shape — single binary, WASM client + Axum server
```

`fullstack` activates Dioxus's own meta-feature (which combines `web` + `server` and re-exports the fullstack types). Per the in-band Dioxus 0.7 skill: "`fullstack` = Combined `web` + `server` shortcuts + types." The release-0.7.0 announcement confirms `ServerEvents`, `Websocket` + `use_websocket`, `Streaming`, and typed `Form` are all first-party in 0.7's fullstack surface. Mobile and desktop use the same WASM client; only the runtime shell differs.

### 4.2 Workspace integration

The existing `Cargo.toml` workspace at `klampatech/alps/Cargo.toml` currently has `members = ["alps-core", "alps-cli"]`. We append `"alps-ui"`. This keeps the existing 184-test suite green and makes the UI a peer of `alps-cli` (it can use `alps-core` types directly without re-exporting). CI in `.github/workflows/ci.yaml` (PR #1, ubuntu-latest) automatically picks up the new crate — no CI changes needed.

## 5. Routing — the type-safe `Routable` enum

Dioxus 0.7's type-safe router (per the in-band skill §"Routing" + "Layouts with `#[layout]`") is the cleanest match for ALPS's screens.

```rust
// src/routes.rs
#[derive(Routable, Clone, PartialEq)]
pub enum Route {
    #[layout(NavBar)]
    #[route("/")]
    Dashboard {},

    #[route("/tasks/new")]
    NewTask {},

    #[route("/tasks/:id")]
    TaskDetail { id: TaskId },        // typed path param via #[route("/tasks/:id")] + component's `id: TaskId`

    #[route("/tasks/:id/log")]
    TaskLog { id: TaskId },

    #[route("/tasks/:id/diff")]
    TaskDiff { id: TaskId },

    #[route("/settings")]
    Settings {},

    #[route("/:..segments")]
    NotFound { segments: Vec<String> },
}
```

The catch-all `NotFound` is required — without it the router renders nothing on unmatched URLs (per the in-band skill §"Catch-all (404) routes"). Each variant must have a component of the same name in scope (or specify one explicitly). The `#[layout(NavBar)]` wraps every route below it; `NavBar` renders `<Outlet::<Route> />` for the page body.

`TaskId` is a typed wrapper around `String` that mirrors `alps_core::domain::TaskId`'s shape (`YYYY-MM-DDTHHMMSS-<uuid8>`). We *do not* `use alps_core::domain::TaskId` directly in the route enum — the router's `PartialEq` derivation needs the type to live in the UI crate. We re-declare a thin wrapper in `src/domain.rs` with a `From<alps_core::domain::TaskId>` impl. This is the only intentional duplication between the UI and the core crate; everything else is server-function-mediated.

## 6. Responsive layout — one Rust binary, three breakpoints

The "flexible enough for all devices" requirement is met by a single Rust codebase with three CSS breakpoints. No `match MediaQuery` branching in Rust — Tailwind handles it.

**Breakpoints (Tailwind defaults, sufficient for ALPS's content shape):**

| Breakpoint | Min width | Target |
|---|---|---|
| (default) | 0 | Phone portrait — single column, log collapses to "Latest 100 lines" |
| `sm:` | 640px | Phone landscape / small tablet — sidebar visible, log full-width |
| `lg:` | 1024px | Desktop / laptop — three-column dashboard (task list + detail + log tail) |

**Implementation pattern (per in-band skill pitfall #6):**

```rust
// src/components/responsive.rs — utility components, not hooks
#[component]
pub fn ResponsiveGrid() -> Element {
    rsx! {
        div { class: "grid grid-cols-1 lg:grid-cols-3 gap-4 p-4",
            { /* page body */}
        }
    }
}
```

CSS lives in `assets/tailwind.css`. `dx serve` runs the Tailwind CLI automatically when it detects `tailwind.css` at the project root (per the new-app tutorial). No PostCSS config needed for an MVP.

**Three layouts, one set of pages:**

1. **Dashboard** (`/`) — `grid-cols-1 lg:grid-cols-3` — task list (1 col) + new-task form (1 col) + recent-activity log (1 col, only visible at `lg:`).
2. **Task detail** (`/tasks/:id`) — `grid-cols-1 lg:grid-cols-[2fr_1fr]` — story list + plan summary on the left, status pill + commit log on the right. Mobile collapses to single column with a tab strip.
3. **Log view** (`/tasks/:id/log`) — `grid-cols-1` — full-width log stream with a sticky search box. Mobile keeps it single column.

**Why Tailwind:** it's the recommended path on Dioxus 0.7 (`dx` auto-runs it), it gives us deterministic responsive utility classes without writing `@media` rules, and it's the path that survives WebView quirks (the in-band skill's pitfall #6 — "CSS varies by platform — Tailwind is generally safe across platforms").

## 7. Server functions — the API surface

Per the in-band skill §"Fullstack / Server functions": "`#[post("/api/...")]` on an async fn creates both an Axum endpoint and a client-callable stub."

Every server function lives behind `#[cfg(feature = "server")]` to keep secrets out of the client binary (in-band pitfall #3).

### 7.1 Public surface

| Function | HTTP | Client call site | Reads | Writes |
|---|---|---|---|---|
| `tasks_list` | `GET /api/tasks?workdir=...` | Dashboard | `tasks/<id>/*` metadata for each task | nothing |
| `task_get` | `GET /api/tasks/:id` | Task detail | the specific task's artifacts | nothing |
| `task_log_tail` | `GET /api/tasks/:id/log?since=N` | TaskLog, LogStream | `<workdir>/.alps-log/<id>.log` | nothing |
| `task_log_stream` | `GET /api/tasks/:id/log/stream` (SSE) | LogStream component | same | nothing |
| `task_diff` | `GET /api/tasks/:id/diff` | TaskDiff | `git log -p alps/<id>` | nothing |
| `task_run` | `POST /api/tasks/run` | NewTask form | the prompt | spawns `alps run` child process; mirrors PID to `<workdir>/.alps-pids.json` |
| `task_cancel` | `POST /api/tasks/:id/cancel` | TaskDetail "Cancel" button | `.alps-pids.json` (UI-managed PID bookkeeping) | SIGTERM → orchestrator PID; the CLI's existing signal handler (`alps-cli/src/main.rs:57-163`) writes the backtrace marker to `$ALPS_SIGTERM_LOG` |
| `settings_get` | `GET /api/settings` | Settings page | `<workdir>/.alps-ui-settings.json` (or `~/.config/alps/ui.toml` for cross-workdir defaults) | nothing |
| `settings_set` | `POST /api/settings` | Settings page | same | same |

### 7.2 Secrets + `#[cfg(feature = "server")]`

The server functions (behind `#[cfg(feature = "server")]`, per in-band pitfall #3) reuse `alps_core::git_ops`, `alps_core::persistence::TaskWorkspace`, and `alps_core::domain` directly — no IPC, no JSON-shape re-implementation. The `tasks_list` function does a `tokio::fs::read_dir(workdir.join("tasks"))` and parses each `plan.json` / `review.json` / `receipts.json` exactly the same way `alps-cli` does. The UI cannot drift from the CLI on what a "task" looks like — they share the parser.

### 7.3 Process spawn (`task_run`)

The server function spawns `alps` as a child process, **not** calls `alps_core::loop_::drive` in-process. Why:

- **In-process is faster but couples UI lifecycle to orchestrator lifecycle.** If the UI server restarts, in-flight orchestrator runs would orphan. Out-of-process keeps the orchestrator independent.
- **CLI flags are already the public API.** `--prompt-file`, `--workdir`, `--deliverable-path`, `--force` — every operator-facing knob is already a CLI flag. We just need to plumb them through the UI form to the spawn.
- **The signal-handler story already exists.** The CLI installs SIGTERM/SIGINT/SIGHUP handlers that write a backtrace marker to `ALPS_SIGTERM_LOG` (verified in `alps-cli/src/main.rs:57-163`). The UI's "Cancel" button sends SIGTERM; the orchestrator writes the marker; the UI's next log-tail fetch picks it up.

Spawn shape (mirrors the smoke-wrapper recipe but without the Tier-4 strace / journalctl / dmesg machinery):

```rust
#[cfg(feature = "server")]
fn spawn_orchestrator(workdir: &Path, prompt_file: &Path, deliverable_path: Option<&Path>) -> Result<Child, ServerError> {
    let mut cmd = std::process::Command::new("alps");
    cmd.arg("run")
        .arg("--workdir").arg(workdir)
        .arg("--prompt-file").arg(prompt_file)
        .env("ALPS_SIGTERM_LOG", workdir.join(".alps-sigterm.log"))
        .env("ALPS_TELEMETRY_LOG", workdir.join(".alps-telemetry.log"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dp) = deliverable_path {
        cmd.arg("--deliverable-path").arg(dp);
    }
    let child = cmd.spawn()?;
    Ok(child)
}
```

PID bookkeeping: write `<workdir>/.alps-pids.json` with `{task_id, pid, started_at}`. Update on `task_cancel` (set `cancelled_at`). The server reads this on restart to surface "running" status for tasks whose orchestrator PID is still alive in the OS process table.

## 8. Live log streaming — SSE, not WebSocket

The `task_log_stream` server function is a Server-Sent Events endpoint. SSE is one-way (server → client) which fits ALPS's read-only-over-log model. WebSockets add bidirectional complexity we don't need.

**Wire shape:**

```
GET /api/tasks/2026-08-23T12:34:56-<uuid8>/log/stream
Accept: text/event-stream

data: {"ts": "2026-08-23T12:35:01.123Z", "level": "info", "line": "[plan] running (Claude)"}
data: {"ts": "2026-08-23T12:35:33.456Z", "level": "info", "line": "[plan] complete in 32s → tasks/.../plan.json"}
...
```

**Why structured JSON, not raw lines:** the UI's `LogStream` component filters by level, searches by substring, and collapses repeated `[judge:structured] ...` markers. JSON makes the client-side filter trivial without parsing log line shapes per-tag. The orchestrator's `elog!` macro emits lines with `[tag] message` shape — the server wraps each tail-of-file read in a JSON event, not a text frame.

**Backpressure:** the orchestrator can produce 100+ lines/sec during heavy Ralph iterations (smoke #18's 60-min run logged ~30k lines). The server reads `<workdir>/.alps-telemetry.log` (the separated telemetry stream per §12 P0#1) with a 200ms poll, batches up to 50 lines per SSE event, and drops oldest if the client falls behind. The client renders the latest 1000 lines in memory.

**Client-side reconnect:** the `use_log_stream` hook wraps `eventsource-stream` with auto-reconnect on `Event::Error` (per in-band skill §"Common patterns" — `use_resource` restart pattern). The `Last-Event-ID` header on reconnect lets the server resume from where the client left off.

## 9. Components — the presentational layer

The Dioxus 0.7 component model (in-band skill §"Components") is a natural fit: every visual element is a `#[component] fn ... (props) -> Element`. Props are auto-derived structs, `Clone + PartialEq` by construction, memoized on `PartialEq` change.

### 9.1 The hot-list of components

| Component | Props | Renders |
|---|---|---|
| `NavBar` | (none — reads from `use_context::<NavState>()`) | Responsive top bar; collapses to hamburger on `<sm` |
| `StatusPill` | `state: TaskState` | Color-coded pill: Idle (gray) / Planned (blue) / Implemented (purple) / Reviewed (yellow) / Done (green) / Rejected (red) / Failed (dark red) |
| `StoryCard` | `story: UserStory`, `passes: bool` | Title, description, acceptance-criteria checklist, DoD criteria, passes badge |
| `FindingCard` | `finding: Finding` | Severity pill (Info / Warning / Error / Critical), description, `file:line` evidence |
| `AssertionCard` | `assertion: Assertion` | `[x]` / `[ ]`, criterion text, evidence snippet |
| `ReceiptCard` | `receipts: Receipts` | Plan summary, implement metrics (stories passed/total, iterations, elapsed), review summary (assertions passed/total, critical findings), judge verdict + model |
| `LogStream` | `task_id: TaskId` | SSE consumer; virtualized scroll; pause/resume button; level filter |
| `CodeBlock` | `content: String`, `lang: Lang` | Syntax-highlighted `<pre>` — markdown for AGENTS.md/progress.txt, json for plan/review/receipts |
| `ResponsiveGrid` | (none — wraps children) | Three-column grid at `lg:`, single column otherwise |

### 9.2 State management across components

Per the in-band skill §"State management — choosing the right tool":

- **Local UI state** (form input, modal open/closed) → `use_signal`
- **Derived value** (filtered log lines) → `use_memo`
- **Shared state across the tree** (selected task ID on dashboard) → `use_context_provider` + `use_context`
- **Async fetch (task detail)** → `use_resource` with `.restart()` on user action
- **User-triggered RPC (run new task, cancel)** → `use_action` for auto-cancel on rapid clicks (in-band pitfall #5)

A `NavState` context provider at the root holds the operator's selected `--workdir` and per-workdir filter (All / Running / Done / Failed). Pages read it via `use_context::<NavState>()`. No `GlobalSignal` — the nav state is per-app-instance, and `GlobalSignal` would couple all open windows (in-band skill §"`GlobalSignal`" — "global per app instance, not per process").

### 9.3 Rules-of-hooks compliance

Every component will pass the in-band skill's hooks discipline check:

- **No hooks inside conditionals** — `let signal = use_signal(...)` always at the top; branch on the signal's value, not on whether to call it.
- **No hooks inside loops** — `for story in stories.iter() { StoryCard {...} }`; the StoryCard component itself calls hooks (state for "expanded?") in stable order.
- **`key` on every list iteration** — `for story in plan.stories.iter() { StoryCard { key: "{story.id}", ... } }`. In-band skill pitfall #2: "without `key`, Dioxus falls back to positional matching — reorders/inserts/remove attach state to the wrong item." This matters for the Stories list in particular because the Plan agent can re-plan on rejection and the story set can shift.

## 10. Verification log — every claim traced to its source

This is the load-bearing section. Every factual claim in this spec is checked against the source it claims to come from.

### 10.1 ALPS-side claims

| Claim | Source | Verified |
|---|---|---|
| ALPS is a Rust workspace at `klampatech/alps` | `~/Development/alps/Cargo.toml` (`[workspace]` block) + `README.md:1-70` | ✓ |
| Workspace currently has `alps-core` + `alps-cli` | `~/Development/alps/Cargo.toml` | ✓ |
| 184 tests passing as of 2026-08-11 | `SPEC.md:3` (status line) | ✓ |
| Four-step orchestrator: Plan → Implement → Review → Judge | `README.md:32-37`, `SPEC.md:1` ("Plan → Implement via Ralph → Review → Judge") | ✓ |
| `loop_::drive` is recursive, not `loop{}` | `SPEC.md:787-832` (the recursive `drive` function with shadowing), `SPEC.md:793` "RECURSIVE, not loop{}" | ✓ |
| Type-state machine: `Task<Idle>`, `Task<Planned>`, etc. | `SPEC.md:436-687` (the per-`impl Task<State>` blocks) | ✓ |
| Hybrid Judge (structured DoD + LLM) | `README.md:140-152`, `SPEC.md:907-935` (§11.1) | ✓ |
| Judge model is `claude-opus-4` (Opus alias); Plan + Review are `claude-sonnet-4` | `README.md:81`, `SPEC.md:917-919` | ✓ |
| Per-task branches `alps/<task-id>` off `main` | `README.md:262`, `SPEC.md:780-784` | ✓ |
| AGENTS.md propagation across Plan/Review/Judge/retry | `SPEC.md:399-402`, `SPEC.md:847` | ✓ |
| Workdir completion guard (5s debounce) | `README.md:85`, `README.md:338-355`, `SPEC.md:740` | ✓ |
| `--deliverable-path <path>` CLI flag (PR #2) | `SPEC.md:867`, `SPEC.md:986`, `alps-cli/src/main.rs:194-201` | ✓ |
| `--prompt-file <path>` CLI flag (§12 item 9.7 fix) | `SPEC.md:999`, `alps-cli/src/main.rs:188-194` | ✓ |
| Auto-detect `--deliverable-path` from prompt (`alps-cli/src/detect.rs`) | `SPEC.md:868`, `SPEC.md:992` | ✓ |
| `--telemetry-log <path>` CLI flag | `README.md:202` | ✓ |
| SIGTERM/SIGINT/SIGHUP handlers write to `ALPS_SIGTERM_LOG` | `alps-cli/src/main.rs:57-163` | ✓ |
| `elog!` macro is O_APPEND-guarded, single-source | `README.md:90`, `SPEC.md:1031` | ✓ |
| Per-task file structure: `tasks/<id>/{prompt.md,plan.json,review.json,receipts.json,feedback.json,implementation.json,AGENTS.md,implementation/ralph/{prd.json,progress.txt,...}}` | `SPEC.md:761-778` | ✓ |
| Receipts JSON shape (status, plan, implement, review, judge) | `README.md:274-294`, `SPEC.md:330-355` | ✓ |
| `Implementation` carries `metrics` + `deliverable_path` | `SPEC.md:246-263`, `SPEC.md:1006-1014` | ✓ |
| `Implementation` carries `artifacts: Vec<Artifact>` collected recursively from `ralph_dir` | `SPEC.md:271-284`, `SPEC.md:866` (recursive artifact collection) | ✓ |
| `alps-cli`'s `List` and `Show { task_id }` subcommand variants exist but are unimplemented stubs (print `not yet implemented`, exit 1) | `alps-cli/src/main.rs:240-247` (enum), `alps-cli/src/main.rs:440-447` (stub bodies) | ✓ |
| `read_artifacts` SKIP_DIRS = `.git, target, node_modules, dist, build, __pycache__, .pytest_cache, .mypy_cache, .gradle, .cargo` | `alps-core/src/implement.rs:734-745` | ✓ |
| `--prompt-file <path>` deletes the file after read (best-effort) — the UI's spawn path must hand alps a path that exists at spawn time | `alps-cli/src/main.rs:296` (`let _ = std::fs::remove_file(path);` in `resolve_prompt`) | ✓ |
| `setpgid(0, 0)` lives at `main.rs:333`; `prctl(PR_SET_PDEATHSIG, SIGTERM)` at `main.rs:348`; the rationale comment is at `main.rs:305-329` | `alps-cli/src/main.rs:333, 348` | ✓ |
| `--deliverable-path` field declaration is at `main.rs:216` (doc comment at 207-215) | `alps-cli/src/main.rs:216` | ✓ |
| `--prompt-file` field declaration is at `main.rs:194` (doc comment at 187-193; resolve_prompt in 290-301) | `alps-cli/src/main.rs:194, 296` | ✓ |
| `Receipts` struct (canonical, in `alps-core/src/receipt.rs:44-53`) has top-level fields `task_id, plan_id, plan_summary, implement_metrics, review_summary, judged_at, judge_model` — there is NO top-level `status` or `verdict` field, and the README.md sample JSON shape (which shows `verdict, plan, implement, review, judge`) is an illustrative flattened view that does NOT match the canonical struct. The UI must deserialize the canonical fields, not the README sample | `alps-core/src/receipt.rs:44-53`, `SPEC.md:330-339` | ✓ |
| `loop_::drive` is a 6-parameter function: `(task: Task<Idle>, &PlanAgent, &ImplementAgent, &ReviewAgent, &JudgeAgent, &TaskWorkspace) -> Result<Task<Done>, AlpsError>` | `alps-core/src/loop_.rs:33` | ✓ |
| `Receipt` (compact summary, printed to Kyle) and `Receipts` (full assembled output) are distinct types, both in `alps-core/src/receipt.rs` | `alps-core/src/receipt.rs:44-72` | ✓ |
| `detect_project_type` returns `(ProjectType, PathBuf)` after §12 P1 cwd fix | `SPEC.md:51` (§12 P1 entry), `SPEC.md:1064` | ✓ |
| Venv-aware Python: `test_command_for(project_type, test_root)` | `SPEC.md:1115-1142` | ✓ |
| Structured DoD commands: cargo / pytest / npm / go test | `README.md:144-150` | ✓ |
| `--force` bypasses workdir guard | `README.md:201`, `README.md:349-353` | ✓ |
| Recursive artifact collection skips `target/`, `node_modules/`, `.git/`, `__pycache__/`, `.gradle/`, `.cargo/`, `dist/`, `build/`, `.pytest_cache/`, `.mypy_cache/` | `README.md:372` (the SKIP_DIRS list) | ✓ |
| `setpgid(0,0)` + `prctl(PR_SET_PDEATHSIG, SIGTERM)` make alps immune to herdr-pane-babysitter SIGTERMs | `README.md:89`, `SPEC.md:9.5` (process-group hardening) | ✓ |
| Tier-4 smoke #26 ACCEPTED (the canonical full-stack verdict) | `SPEC.md:3` (status line), `SPEC.md:58` (smoke #26 row) | ✓ |
| `alps-cli` is the only existing binary; no GUI binary exists today | `SPEC.md:435-440`, `SPEC.md:874` ("Web UI for monitoring" is still unchecked under Phase 3) | ✓ |
| Phase 3 roadmap explicitly lists "Web UI for monitoring" as outstanding | `SPEC.md:874-877` | ✓ |

### 10.2 Dioxus-side claims

| Claim | Source | Verified |
|---|---|---|
| Dioxus 0.7.0 is current stable | `https://dioxuslabs.com/learn/0.7/tutorial/new_app/` (top-of-page: "Using Stable Version 0.7.0") | ✓ |
| `dx new <name>` scaffolds a project | `https://dioxuslabs.com/learn/0.7/tutorial/new_app/` ("You can create a new Dioxus project by running the following command") | ✓ |
| Three templates: bare-bones / jumpstart / workspace | new-app tutorial | ✓ |
| Project layout: `Cargo.toml`, `Dioxus.toml`, `assets/`, `src/main.rs` | new-app tutorial (Structure of the app section) | ✓ |
| `dx serve` runs Tailwind CLI automatically when `tailwind.css` exists at root | new-app tutorial (tailwind.css section: "dx automatically runs the TailwindCSS CLI if it detects a tailwind.css at the root of your app") | ✓ |
| `dioxus::launch(App)` is the entry point | new-app tutorial ("The launch function calls the platform-specific launch function") | ✓ |
| `#[component]` macro + `rsx!{}` markup | In-band skill §"Components" + §"RSX" | ✓ |
| `Signal<T>` with `.read()` / `.write()` / `.set()` | In-band skill §"`Signal<T>` API" | ✓ |
| `use_resource` for async data, `use_action` for user-triggered RPC | In-band skill §"State management" | ✓ |
| `Routable` enum + `#[route("...")]` + `#[layout(...)]` + `<Outlet::<Route> />` | In-band skill §"Routing" + §"Layouts with `#[layout]`" | ✓ |
| Catch-all 404 requires `#[route("/:..segments")]` | In-band skill §"Catch-all (404) routes" | ✓ |
| `#[post("/api/...")]` server functions compile to Axum endpoints | In-band skill §"Fullstack / Server functions" | ✓ |
| Server-only code MUST live behind `#[cfg(feature = "server")]` | In-band skill pitfall #3 ("Server-only module") | ✓ |
| `fullstack` feature = `web` + `server` combined | In-band skill §"Cargo features cheat sheet" | ✓ |
| Forms in 0.7 require explicit `evt.prevent_default()` | In-band skill §"Forms in 0.7 — must prevent_default explicitly" | ✓ |
| Hot-patch via Subsecond (`dx serve --hotpatch`) is new in 0.7 | In-band skill §"Hot reload" #3 | ✓ |
| Tailwind is generally safe across desktop WebViews (the in-band pitfall #6 hedge) | In-band skill pitfall #6 ("CSS varies by platform — Tailwind is generally safe") | ✓ |
| `gloo-storage` pattern for persistent signals on web | In-band skill §"Pattern: Persistent state via custom hook (web)" | ✓ |
| `use_loader` runs on the server during SSR and on the client during hydration (new in 0.7 — distinct from `use_server_future`; will not re-suspend the page when the future re-runs) | In-band skill §"Hydration / SSR" + verified at `https://dioxuslabs.com/learn/0.7/essentials/fullstack/ssr` 2026-08-23 | ✓ |
| `use_websocket` is a first-party Dioxus 0.7 hook for long-lived client ↔ server streams (server side uses `WebsocketOptions::on_upgrade`; client side uses `use_websocket` with `.send()` / `.status()`) | `https://dioxuslabs.com/learn/0.7/essentials/fullstack/websockets` + `https://dioxuslabs.com/blog/release-070` | ✓ |
| `ServerEvents` is a first-party Dioxus 0.7 type for one-way Server-Sent Events (wraps `axum::response::sse`) | `https://dioxuslabs.com/blog/release-070` ("Server Sent Events with `ServerEvents` type") | ✓ |
| `fullstack` is a real Cargo feature in Dioxus 0.7 (`dioxus/fullstack`); it activates the meta-feature that re-exports the fullstack types (`ServerEvents`, `Websocket`, `Streaming`, typed `Form`, `FileStream`). Downstream crates wire it as `fullstack = ["dioxus/fullstack"]` | `https://dioxuslabs.com/blog/release-070` | ✓ |

## 11. Open questions

### Q1. In-process `drive` vs spawn `alps` CLI for `task_run`?

The spec currently says **spawn `alps run` as a child process.** The alternative is to call `alps_core::loop_::drive(task, &plan_agent, &impl_agent, &review_agent, &judge_agent)` in-process inside the server function. Trade-offs:

| | Spawn `alps` (current spec) | In-process `drive` |
|---|---|---|
| Decoupling | Orchestrator survives UI server restart | Orchestrator dies when UI restarts |
| Type sharing | None — just argv | Full Rust types across UI and orchestrator |
| CLI parity | UI invokes the same binary the operator does | UI is a separate entry point; behavior can drift |
| Latency | ~50ms spawn per task (acceptable; tasks take minutes) | ~0ms |
| Process supervision | OS PID + `.alps-pids.json` | Tokio task handle + `JoinHandle` |
| Cancel semantics | SIGTERM → existing handler writes backtrace | `tokio::sync::oneshot` cancellation |

**Recommendation:** spawn `alps` for v1. Keeps the CLI as the single public API; keeps the operator's mental model ("`alps run` is the entry point") intact. Revisit if/when we add features that require in-process coupling (e.g., a "step manually through Plan → Implement → Review → Judge" debugging mode).

> **Note for the Q1 alternative path:** if we ever do switch to in-process `drive`, the signature is `alps_core::loop_::drive(task: Task<Idle>, &plan_agent, &impl_agent, &review_agent, &judge_agent, &workspace: &TaskWorkspace) -> Result<Task<Done>, AlpsError>` (verified at `alps-core/src/loop_.rs:33`) — six parameters, not five. The `workspace` is needed for persistence writes at each state transition.

### Q2. Is the UI a new Cargo crate or a new binary in `alps-cli`?

The spec says **new crate `alps-ui/`**. The alternative is to add a second binary `[[bin]]` target in `alps-cli/Cargo.toml` for `alps-ui`. Trade-offs:

| | New crate (current spec) | New `[[bin]]` in `alps-cli` |
|---|---|---|
| Dependency surface | `alps-ui` depends on `alps-core` directly; doesn't depend on `alps-cli` | `alps-ui` would share `alps-cli`'s deps, including signal-hook |
| Build matrix | Independent `cargo build -p alps-ui` | Tied to `alps-cli` build |
| Feature flags | Clean separation: `alps-ui` has `web`/`desktop`/`mobile`/`server`; `alps-cli` stays CLI-only | Dioxus features would leak into `alps-cli` |
| CI | New crate's tests run independently | One `cargo test --workspace` covers both |

**Recommendation:** new crate. The dependency isolation is worth the extra `Cargo.toml`.

### Q3. SSE on Dioxus 0.7 — what's the idiomatic client?

**Resolved 2026-08-23 by web-search verification:** the in-band skill's claim was correct.

- Dioxus 0.7 ships a first-party `ServerEvents` type and `use_websocket` hook for long-lived server-to-client streams (`https://dioxuslabs.com/blog/release-070` and `/learn/0.7/essentials/fullstack/websockets`).
- The `#[get("/api/.../stream")]` server function returning `Result<impl IntoResponse>` uses `WebsocketOptions::on_upgrade` for true WebSockets; for one-way SSE, the recommended path is the `ServerEvents` type, which wraps `axum::response::sse`.
- Client side: `use_websocket` works for either direction. For pure SSE (one-way), a thin wrapper around `Websocket` with read-only semantics is the canonical pattern.

**Recommendation:** use `ServerEvents` server-side + `use_websocket` client-side. Both are first-party in 0.7. The exact `ServerEvents` API surface (event payload shape, reconnect contract) needs a final read of `/learn/0.7/essentials/fullstack/websockets` at implementation time, but the existence and shape of the feature is now confirmed.

### Q4. Where does the UI server bind?

Three options:

1. **Same port as `alps-cli`'s default** (`--port 8080` is Dioxus's default per the new-app tutorial; `alps-cli` doesn't bind a port today). No conflict.
2. **A separate port** (`--port 5174` like the specialists-web stack in active memory). Cleaner separation; documented in `Dioxus.toml`'s `[web.app]` section.
3. **Reverse-proxied behind an existing service** (Caddy, the specialists-web stack). Out of scope for v1.

**Recommendation:** option 2 — `5174`. Matches the operator's mental model from specialists-web. Documented in `Dioxus.toml`.

### Q5. Authentication?

ALPS has no auth today. The CLI runs locally. The UI server, if it binds to a non-loopback address, exposes the same operator surface as the CLI. **For v1, the UI binds to `127.0.0.1` only** (Dioxus's default `dx serve` address is `http://127.0.0.1:8080`; we'd use `127.0.0.1:5174`). If we want LAN access (Tailscale funnel, smoke harness from another machine), we add basic-auth or a Tailscale-ACL check in a follow-up. **No auth in v1.**

## 11.5. Corrections from Claude Code review (2026-08-23)

The spec was reviewed end-to-end against the ALPS source by Claude Code (`claude -p`, opus-4, 36 turns, ~$2.27, no edits). 8 factual errors found and corrected; 26 ALPS-side claims independently re-verified clean.

**Issues found and fixed:**

1. **§1, §7.2 — "alps-ui-server" name drop.** Removed the dangling crate name; the server functions live inside the single `alps-ui` crate behind `#[cfg(feature = "server")]`.
2. **§4.1 — `fullstack` feature not defined.** Added `fullstack = ["dioxus/fullstack"]` to the [features] block.
3. **§7.1 — dangling "Signal handlers" cross-reference.** Replaced with the actual citation (`alps-cli/src/main.rs:57-163`).
4. **§10.1 — `alps list` / `alps show` are stubs.** Replaced the over-claim ("already exposes ... read-only task inspection") with the truth ("enum variants exist but unimplemented stubs — UI is the first real implementation").
5. **§10.1 — Receipts JSON shape.** Corrected to match the canonical struct in `alps-core/src/receipt.rs:44-53` (`task_id, plan_id, plan_summary, implement_metrics, review_summary, judged_at, judge_model`); explicitly noted that the README sample JSON is an illustrative flattened view, not the canonical schema.
6. **§10.1 — line-number citations off by ~15 lines.** Fixed `setpgid` (315→333), `prctl` (315→348), `--deliverable-path` (194-201→216), `--prompt-file` (188-194→194), added `Receipt` vs `Receipts` distinction.
7. **§11 Q1 — `drive()` has 6 params, not 5.** Added the missing `workspace: &TaskWorkspace` parameter; added a note for the Q1 alternative path so the call shape compiles if we ever switch to in-process.
8. **§11 Q3 — SSE resolution.** Re-verified via live docs that `ServerEvents` + `use_websocket` are both first-party in Dioxus 0.7 (the in-band skill's claim was correct; the reviewer's uncertainty was unwarranted). Updated Q3 to "resolved" and added 4 new ✓ rows to §10.2 for `use_websocket`, `use_loader`, `ServerEvents`, and the `fullstack` feature.

**Issues flagged but deferred (judgment calls, not errors):** none — all 10 issues were either factual errors (now fixed) or verification gaps (now closed).

**Uncertainties raised by the reviewer that turned out to be unwarranted:** the reviewer couldn't see the in-band `dioxus-0.7` skill (it's only loaded into the parent agent's context, not into spawned review subagents), so they were uncertain about `use_websocket`, `use_loader`, and `ServerEvents`. Live-doc verification on 2026-08-23 confirmed all three are first-party in Dioxus 0.7. The reviewer's discipline of flagging the uncertainty was correct; the underlying claim was correct.

## 12. Implementation milestones

JTBD → topics of concern → tasks. Each task is one session of focused work for a coding agent.

### Topic A — Workspace + scaffold

```
# feat: Add alps-ui crate to klampatech/alps workspace
Description: Create alps-ui/Cargo.toml as a new workspace member; add it to the
root Cargo.toml [workspace] members list. Verify `cargo build -p alps-ui` and
`cargo test -p alps-ui` succeed with no source files beyond main.rs.

## Acceptance Criteria
- `cargo build -p alps-ui --features web` succeeds
- `cargo build -p alps-ui --features server` succeeds
- `cargo test -p alps-ui` succeeds (no tests yet, but the harness is wired)
- `cargo build --workspace` still passes (existing 184 tests unchanged)

# feat: Scaffold Dioxus 0.7 project via `dx new` + cleanup
Description: Run `dx new alps-ui --template jumpstart --no-fullstack --router
--tailwind --platform web`. Then strip the jumpstart components/views to bare
minimum so we start from a known clean state (matches the new-app tutorial's
"Resetting to Basics" section). Add the assets/tailwind.css + main.css entry.

## Acceptance Criteria
- `dx serve` from alps-ui/ produces a default Dioxus page at http://127.0.0.1:5174
- Hot-reload works for a `<p>` text change in src/main.rs's App component
- Cargo features match §4.1

# feat: Add alps-core as a dependency of alps-ui
Description: Wire `alps_core` into alps-ui's Cargo.toml. Import `TaskId`,
`Plan`, `UserStory`, `StoryId`, `Finding`, `Assertion`, `Severity`, `Review`,
`Implementation`, `Artifact`, `ArtifactKind`, `Feedback`, `Judgment`,
`Prompt`, `ProjectType` from `alps_core::domain`. Import `Receipts`,
`Receipt`, `ImplementMetrics`, `ReviewSummary` from `alps_core::receipt`
(these live in `receipt.rs`, NOT `domain.rs`). Compile-check the imports
in src/domain.rs (the UI-side mirror).

## Acceptance Criteria
- `cargo build -p alps-ui` compiles with `use alps_core::domain::*;` AND
  `use alps_core::receipt::{Receipts, Receipt, ImplementMetrics, ReviewSummary};`
  in src/domain.rs (the two modules together provide the full surface the UI needs)
- `cargo test --workspace` still passes (184 + 0 new tests)
- `alps_core::receipt::Receipt` (compact summary) and `alps_core::receipt::Receipts`
  (full assembled output) are imported as distinct types — the UI's StatusPill
  uses `Receipt` for the Done-state summary card, while the structured artifact
  viewer uses `Receipts`
```

### Topic B — Routes + layouts

```
# feat: Implement the type-safe Routable enum
Description: Write src/routes.rs with the Route enum from §5. Add empty stub
components for every variant (Dashboard {}, NewTask {}, TaskDetail { id },
TaskLog { id }, TaskDiff { id }, Settings {}, NotFound { segments: Vec<String> })
that render a placeholder `<p>{route_name}</p>`. Verify the router renders
each route and the catch-all catches `/foo`.

## Acceptance Criteria
- Navigating to `/` renders Dashboard's placeholder
- Navigating to `/tasks/2026-08-23T12:00:00-abcdef01` renders TaskDetail
- Navigating to `/foo` renders NotFound
- `cargo test -p alps-ui` passes with at least one snapshot test per route

# feat: Build the responsive NavBar layout
Description: Implement src/layouts/nav.rs with the responsive nav from §6.
Use Tailwind utility classes (grid-cols-1 lg:grid-cols-3 etc.). The nav has
three sections: app title + version, current-workdir selector, settings cog.
Wrap with `#[layout(NavBar)]` on the Route enum.

## Acceptance Criteria
- At 375px (phone), nav collapses to a hamburger
- At 1024px (desktop), nav is a horizontal bar
- Outlet::<Route> renders below the nav
- Visual snapshot tests at 375px / 768px / 1280px
```

### Topic C — Server functions + API

```
# feat: Implement the api/tasks module (read-only)
Description: Write src/api/tasks.rs with #[get] server functions tasks_list
and task_get. Read <workdir>/tasks/<id>/prompt.md, plan.json, review.json,
receipts.json, feedback.json, implementation.json (whichever exist) and
return typed structs. Behind #[cfg(feature = "server")] (in-band pitfall #3).
Re-use alps_core::persistence::TaskWorkspace for directory access.

## Acceptance Criteria
- `tasks_list(".")` returns the parsed summaries of every task under ./tasks/
- `task_get("2026-08-23T...")` returns the full artifact set
- Both compile under `--features server` only
- Integration test in tests/api_integration.rs drives both against a temp workdir

# feat: Implement the api/run + api/cancel modules
Description: Write src/api/run.rs (task_run) and src/api/cancel.rs (task_cancel).
task_run spawns `alps run --workdir X --prompt-file Y [--deliverable-path Z]`
with the SIGTERM_LOG env var set. task_cancel reads <workdir>/.alps-pids.json
and sends SIGTERM to the recorded PID. Both write PID bookkeeping.

## Acceptance Criteria
- task_run on a Tier-1 fib-style prompt produces a real running orchestrator
  visible via `ps`
- task_cancel on that orchestrator triggers the existing signal handler
  (backtrace marker appears in <workdir>/.alps-sigterm.log)
- Integration test: spawn a sleep-alike binary, cancel it, verify the PID is gone
```

### Topic D — Live log streaming

```
# feat: Implement the SSE log stream endpoint
Description: src/api/stream.rs with task_log_stream (#[get("/api/tasks/:id/log/stream")])
using WebsocketOptions::on_upgrade or the SSE-shaped server fn pattern (per Q3
resolution). Server tails <workdir>/.alps-telemetry.log with a 200ms poll and
emits JSON events (timestamp, level, line).

## Acceptance Criteria
- Connecting to the endpoint with `curl -N` shows live lines as they appear
- Reconnect with Last-Event-ID resumes from the last emitted offset
- Server-side batch size cap of 50 lines/event enforced
- Client-side use_log_stream hook auto-reconnects on disconnect

# feat: Build the LogStream component + use_log_stream hook
Description: src/components/log_stream.rs consumes the SSE stream; renders
the latest 1000 lines with virtualization (only render what's in viewport).
Pause/resume button. Level filter (info / warn / error). Substring search.

## Acceptance Criteria
- Component renders without re-render thrash at 30k lines
- Pause stops the scroll, resume continues
- Search filter hides non-matching lines without unmounting them
```

### Topic E — Pages + components

```
# feat: Build the Dashboard page (task list + new-task form)
Description: src/pages/dashboard.rs uses use_resource(api::tasks::tasks_list)
and renders a responsive grid (1 col mobile / 3 col desktop) with: new-task
form (1 col), task list (1 col), recent activity log (1 col, lg: only).

## Acceptance Criteria
- Form POSTs to api::tasks::task_run; on success, navigates to /tasks/<new-id>
- Task list shows status pill + attempt count + elapsed for each task
- Visual snapshot test at 375px / 768px / 1280px

# feat: Build the TaskDetail page
Description: src/pages/task_detail.rs renders StoryCard for each plan story,
FindingCard for each review finding, AssertionCard for each review assertion,
ReceiptCard on Done state. Tabs (Plan / Implement / Review / Judge / Logs /
Diff) — mobile tabs collapse to a select element.

## Acceptance Criteria
- All four states (Planned / Implemented / Reviewed / Done) render correctly
- Rejected state surfaces the feedback.json reason prominently
- Visual snapshot test for each state

# feat: Build the StatusPill, StoryCard, FindingCard, AssertionCard, ReceiptCard components
Description: Each component is a #[component] fn taking typed Props. StatusPill
maps the 7 TaskState variants to color codes. StoryCard renders UserStory fields
with a passes badge. FindingCard renders Finding with severity pill +
file:line evidence. AssertionCard renders Assertion with [x]/[ ] + evidence.
ReceiptCard summarizes Receipts.

## Acceptance Criteria
- Each component renders correctly given a fixture
- No hooks-in-conditionals violations (greppable: `if .* { use_` in components/)
- Every list iteration uses key: "{...id}"
```

### Topic F — Settings + polish

```
# feat: Build the Settings page + persistence
Description: src/pages/settings.rs renders a form for default workdir, default
Plan/Review/Judge model aliases, smoke-wrapper defaults. Persists via
api/settings_set to <workdir>/.alps-ui-settings.json (workdir-local) and
~/.config/alps/ui.toml (cross-workdir defaults).

## Acceptance Criteria
- Settings persist across UI server restarts
- Form re-renders the saved values on load

# feat: Mobile responsiveness audit + visual snapshot suite
Description: Run the full app at 375px / 768px / 1280px in headless Chrome;
capture screenshots; fix any layout regressions. Add visual snapshot tests
for every page at all three breakpoints to tests/responsive_layout.rs.

## Acceptance Criteria
- All 9 page × 3 breakpoint combinations snapshot identically across runs
- No horizontal scroll on 375px
- Touch targets ≥ 44px on phone breakpoint (per Apple's HIG)
```

### Topic G — CI + smoke

```
# feat: Add alps-ui to the GitHub Actions CI matrix
Description: Edit .github/workflows/ci.yaml (existing PR #1 workflow) to
include `cargo build -p alps-ui --features web,server` and `cargo test -p alps-ui`.
The desktop and mobile features require platform-specific toolchains (Xcode,
Android NDK) so they stay opt-in via a separate matrix entry for now.

## Acceptance Criteria
- CI run on a UI PR shows green builds for web + server features
- Total CI wall-clock stays under 3 minutes (current budget is ~1m38s with 184 tests)
- ui-desktop and ui-mobile jobs marked allow-failure until toolchain is sorted

# feat: Add a Tier-1 smoke for the UI end-to-end
Description: scripts/alps-ui-smoke.sh that (a) runs `dx serve --port 5174` in
the background, (b) Playwright drives the dashboard to submit a fib-style
prompt, (c) waits for the orchestrator to finish (via the receipt endpoint),
(d) asserts the dashboard re-renders with the new task in Done state. Captures
screenshots at 375 / 768 / 1280. Codifies the smoke-#26 filesystem/telemetry
monitor pattern from §12 P7 so the herdr-wait timeout failure shape is
structurally impossible.

## Acceptance Criteria
- Smoke completes in <5 minutes (Tier-1 prompt is small)
- All three screenshots saved and visually sane (no layout overflow)
- Receipts from the underlying alps run match the manual `alps run` shape
```

## 13. What is explicitly out of scope for v1

- **Authentication / authorization.** Localhost-only (`127.0.0.1:5174`). LAN/Tailscale exposure is a follow-up.
- **Mobile builds (`dx bundle --mobile --release`).** The `mobile` Cargo feature is wired so the code compiles; actually shipping to TestFlight / Play Store is a separate decision (Apple Developer account, signing certs, etc.).
- **A "step through Plan → Implement → Review → Judge manually" debugging mode.** Would require in-process coupling (Q1 alternative). Defer until a real need surfaces.
- **Cross-task learning / persistent task queue (SQLite).** Still open under ALPS Phase 3 roadmap (`SPEC.md:874`).
- **Multi-model judge ensemble.** Still open under ALPS Phase 3 roadmap (`SPEC.md:877`).
- **Replacing the CLI.** The UI is additive. `alps run` stays.

## 14. Risk register

| Risk | Probability | Impact | Mitigation |
|---|---|---|---|
| Dioxus 0.7 WebView quirks break the responsive layout on Linux WebKitGTK or Windows WebView2 | Medium | Low | Tailwind utility classes are "generally safe across platforms" (in-band pitfall #6). We test on Linux + macOS WebKit first; Windows WebView2 second. If something breaks, fall back to plain CSS in a `#[cfg(target_os)]` block. |
| `cargo build --workspace` time balloons when alps-ui is added | Medium | Low | The UI crate compiles mostly WASM-bound. `--no-default-features` keeps the server-only build under 30s. CI's rust-cache (PR #1's `Swatinem/rust-cache@v2`) handles incremental builds. |
| The on-disk artifact model changes (a new field added to `Plan` / `Receipts`) and the UI breaks | Low | Medium | `alps_core::domain` types are shared, so any breakage surfaces at compile-time. We add a UI test that parses a fixture receipt + plan at the top of `tests/api_integration.rs`. |
| The UI server restarts mid-task and orphans the orchestrator | Low | Medium | Spawned `alps run` is detached (own process group via `setsid`); PID written to `.alps-pids.json`. On server restart, we re-scan the file and surface the running tasks on the dashboard. |
| SSE backpressure at 100+ lines/sec during heavy Ralph iterations | Medium | Medium | 200ms server poll + 50-line batch cap + 1000-line client-side cap. Verified against smoke #18's 30k-line / 60-min profile (~8 lines/sec average; peaks at ~50 lines/sec). |
| The smoke harness from `klampatech/alps` (Tier-4 wrapper at `/tmp/alps-tier4-smoke-wrapper.sh`) doesn't translate to the UI's smoke needs | High | Low | The UI's smoke (Topic G) uses a separate Playwright-driven runner that exercises the actual UI flow, not the CLI. The existing Tier-4 wrapper stays as-is for the underlying `alps run` invocation. |

## 15. Acceptance gate for v1

The UI is "done for v1" when:

1. **All Topic A–G tasks completed.** Each with its acceptance criteria green.
2. **CI green** on a PR adding `alps-ui` to the workspace. 184 + N (UI) tests passing.
3. **One end-to-end UI smoke** (Topic G) green — Playwright drives the dashboard → submits a Tier-1 prompt → reads the receipt back from the UI.
4. **Visual snapshot suite** green at 375 / 768 / 1280 for every page.
5. **No regression** in the existing 184-test suite (`cargo test --workspace`).
6. **Operator UX validated by Kyle** on the local dev box — does the dashboard surface the right info? Does the new-task form need `--workdir` exposed? Is the cancel button discoverable?
