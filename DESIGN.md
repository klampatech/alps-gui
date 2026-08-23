# ALPS UI Design Constraints — Addendum to SPEC.md

> **Purpose.** This document is referenced by the ALPS smoke prompt so the
> orchestrator doesn't invent the design. It is **not** a new design
> system — it's a deliberately small, opinionated baseline that says
> "use these tokens, these breakpoints, these components, this layout."
> Anything not specified here, ALPS can decide. Anything specified
> here, ALPS should follow.
>
> **Date:** 2026-08-23 · **Source of truth:** `klampatech/alps-gui` repo
> (this file will land alongside the scaffold). For the smoke, the
> full content of this file is inlined into the prompt that hands the
> task to ALPS, so the orchestrator doesn't need repo access.

## 1. What this UI is for

A read-mostly dashboard for inspecting ALPS task state. The user
(Kyle) wants to see "what's running, what just finished, what failed"
at a glance — not edit prompts, not configure agents, not run new
tasks beyond typing one and pressing a button. Every screen should
answer one of three questions:

1. **What's running?** — Dashboard + active task's log.
2. **What happened on task X?** — Task detail (plan / implementation
   / review / receipts / failure feedback).
3. **How do I run a new one?** — A form at the top of the dashboard.

Everything else is out of scope. No settings page, no auth, no
multi-workdir switcher. (Those land in v2.)

## 2. Visual language

### Color palette

Use a small set of named colors via Tailwind utilities. **No
hex codes.** No inline styles. Tailwind classes only.

| Token | Tailwind class | When |
|---|---|---|
| Page background | `bg-slate-50` | Every page |
| Card background | `bg-white` | Cards, list rows |
| Border | `border-slate-200` | Subtle dividers |
| Body text | `text-slate-800` | Default |
| Muted text | `text-slate-500` | Timestamps, metadata |
| Accent | `bg-indigo-600` | Primary buttons, links, active nav |
| Success | `bg-emerald-500` | "Done" pill |
| Warning | `bg-amber-500` | "Running" pill, "Reviewed" pill |
| Error | `bg-rose-500` | "Rejected" pill, "Failed" pill |
| Neutral | `bg-slate-400` | "Idle", "Planned", "Implemented" pills |

### Typography

- One font family: Tailwind's default sans (system stack).
- Page title: `text-2xl font-semibold`.
- Section title: `text-lg font-medium`.
- Body: default size (`text-base`).
- Metadata (timestamps, paths): `text-sm text-slate-500`.
- Monospace (task IDs, file paths in artifacts): `font-mono text-sm`.

### Spacing

- Page padding: `p-4 sm:p-6 lg:p-8`.
- Card padding: `p-4`.
- Stack gap (vertical list of cards): `space-y-3`.
- Inline gap (button group, pill row): `gap-2`.

### Borders / shadows

- Cards: `rounded-lg border border-slate-200 shadow-sm`.
- Pills (status, severity): `rounded-full px-2.5 py-0.5 text-xs font-medium text-white`.

## 3. Layout breakpoints

Three breakpoints, single Rust codebase, Tailwind utility classes
drive everything. No JS-side responsive logic.

| Breakpoint | Width | Layout |
|---|---|---|
| Default | 0+ | Single column. Nav collapses to a hamburger (`< sm`). Log view is full-width. |
| `sm:` | ≥640px | Nav is horizontal. Task list and detail both stack vertically. |
| `lg:` | ≥1024px | Dashboard becomes 3-column (task list + new-task form + recent activity). Task detail becomes 2-column (stories left, summary right). Log tail pinned to bottom-right. |

Test at exactly **375px, 768px, 1280px**. At 375px, no horizontal
scroll on any page. Touch targets ≥ 44px.

## 4. Components — required shapes

These are the named components the SPEC references. Use these exact
names so the SPEC's component map matches what's in the code.

### StatusPill

A pill that renders the current task state. One of 9 possible
states — see the SPEC's `TaskState` enum.

```rust
#[component]
fn StatusPill(state: TaskState) -> Element {
    let (label, bg) = match state {
        TaskState::Running => ("Running", "bg-amber-500"),
        TaskState::Idle => ("Idle", "bg-slate-400"),
        TaskState::Planned => ("Planned", "bg-slate-400"),
        TaskState::Implemented => ("Implemented", "bg-slate-400"),
        TaskState::Reviewed => ("Reviewed", "bg-amber-500"),
        TaskState::Done => ("Done", "bg-emerald-500"),
        TaskState::Rejected => ("Rejected", "bg-rose-500"),
        TaskState::Failed => ("Failed", "bg-rose-700"),
        TaskState::Unknown => ("Unknown", "bg-orange-500"),
    };
    rsx! {
        span { class: "rounded-full px-2.5 py-0.5 text-xs font-medium text-white {bg}",
            "{label}"
        }
    }
}
```

### StoryCard

One user story from `Plan.stories`. Renders title, description,
acceptance criteria checklist, and a pass/fail indicator if
`prd.json` has `passes: true|false` for this story.

```rust
#[component]
fn StoryCard(story: UserStory, passes: Option<bool>) -> Element {
    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm",
            div { class: "flex items-start justify-between",
                h3 { class: "text-base font-medium text-slate-800", "{story.title}" }
                {passes.map(|p| rsx! {
                    span { class: if p { "rounded-full bg-emerald-500 px-2 py-0.5 text-xs text-white" }
                                else { "rounded-full bg-slate-300 px-2 py-0.5 text-xs text-white" },
                        if p { "Pass" } else { "Pending" }
                    }
                })}
            }
            p { class: "mt-2 text-sm text-slate-600", "{story.description}" }
            ul { class: "mt-3 space-y-1",
                for ac in story.acceptance_criteria.iter() {
                    li { class: "flex items-start gap-2 text-sm text-slate-700",
                        span { class: "mt-1 h-1.5 w-1.5 rounded-full bg-slate-400" }
                        span { "{ac}" }
                    }
                }
            }
        }
    }
}
```

### FindingCard / AssertionCard / ReceiptCard

Render one Review finding, one Review assertion, one final Receipt.
Use the same card pattern as StoryCard. Findings show severity via
the same pill palette (Info=`bg-slate-400`, Warning=`bg-amber-500`,
Error=`bg-rose-500`, Critical=`bg-rose-700`). Assertions show
[x]/[ ] glyphs. Receipts show the canonical Receipts fields
(implement_metrics, review_summary, judge_model, plan_summary).

### LogStream

A virtualized list (only render what's in viewport) that consumes
the SSE stream and shows the latest 1000 lines with a pause/resume
button and a substring search input. See SPEC §8 for the wire shape.

### ResponsiveGrid

A `ResponsiveGrid` wrapper component that applies the layout for each
page (single column on default, three-column on `lg:`). Implemented
as a `div` with the appropriate Tailwind grid classes — no Rust-side
breakpoint logic.

## 5. Pages

### Dashboard (`/`)

```text
lg (3 cols):
┌──────────────┬──────────────┬──────────────┐
│ Task list    │ New task     │ Recent log   │
│ (cards)      │ form         │ tail         │
│              │              │              │
└──────────────┴──────────────┴──────────────┘

default (1 col):
┌──────────────┐
│ New task     │
│ form         │
├──────────────┤
│ Task list    │
│ (cards)      │
├──────────────┤
│ Recent log   │
│ tail         │
└──────────────┘
```

- New-task form: single textarea + Submit button. POSTs to
  `task_run` server function with the prompt text.
- Task list: each task is a card with status pill, prompt excerpt,
  attempt count, and elapsed time. Click navigates to `/tasks/:id`.
- Recent log: shows the latest 200 lines from the most-recently-
  active task's log stream.

### Task detail (`/tasks/:id`)

Tabs (mobile: bottom-anchored, desktop: top-anchored): **Plan** /
**Implement** / **Review** / **Judge** / **Logs** / **Diff**.

```text
lg (2 cols):
┌──────────────────────────┬──────────────┐
│ Stories / findings /     │ Summary      │
│ assertions               │ (state,      │
│                          │ attempts,    │
│                          │ metrics,     │
│                          │ Cancel btn)  │
└──────────────────────────┴──────────────┘
```

The summary card on the right is always visible (sticky on desktop).
It contains: StatusPill, attempt count, created_at, completed_at,
and — if Done — the Receipts metrics. The Cancel button only shows
for non-terminal states.

### New task (`/tasks/new`)

Standalone page that's the new-task form on its own (alternative
entry-point if the user doesn't want the dashboard form). Same
form, same submission. After submit, navigate to `/tasks/:id`.

### Log view (`/tasks/:id/log`)

Full-width LogStream component with a sticky search box at the top.
Pause button stops scroll but keeps the stream connected.

### Diff view (`/tasks/:id/diff`)

A `git log -p alps/<id>` rendered in a `<pre>` block with
`whitespace-pre-wrap`. Monospace, small text, scrollable container.

### Settings (`/settings`)

Stub page for v1 — renders "Settings coming in v2" + a link back to
the dashboard. We can flesh this out once the core surfaces are
shipped.

### Not found (catch-all)

A simple 404 page with "Task not found" and a link back to the
dashboard. Use the same card pattern as everything else.

## 6. Accessibility

- All interactive elements have an accessible name (the `aria-label`
  attribute when the visible label is ambiguous).
- StatusPill has `role="status"`.
- The Cancel button requires a confirm dialog (`role="alertdialog"`)
  before submitting.
- Color is never the only signal — pills include text labels too.
- Keyboard navigation works through every page (no `tabindex`
  overrides; rely on source order).

## 7. What is explicitly NOT in v1

- **Real-time WebSocket / SSE log streaming.** SPEC §8 describes it.
  The smoke builds the dashboard with **static log content** (last
  200 lines from `<workdir>/.alps-telemetry.log` via a one-shot
  fetch). SSE/WebSocket is a follow-up — it requires the orchestrator
  to write `.alps-pids.json` so the server knows which task is
  active.
- **Cancellation.** The Cancel button is rendered but no-op.
- **Settings page content.**
- **Mobile builds (`dx bundle --mobile --release`).** The mobile
  Cargo feature compiles, but no actual shippable mobile artifact.
- **Authentication / authorization.** Localhost only.

The smoke prompt explicitly calls out the deferred items so the
LLM doesn't try to implement them.

## 8. Verification checklist (for the orchestrator's Review agent)

The smoke prompt tells ALPS that the Review agent should look for:

- [ ] All 6 components (`StatusPill`, `StoryCard`, `FindingCard`,
      `AssertionCard`, `ReceiptCard`, `LogStream`) compile and
      render with the props given.
- [ ] At least one card per page renders without overflow at 375px,
      768px, 1280px.
- [ ] StatusPill renders all 9 `TaskState` variants without crashing.
- [ ] `cargo build --workspace` succeeds.
- [ ] `cargo build --workspace --bin alps-ui --features fullstack`
      succeeds (fullstack = web + server combined).
- [ ] `cargo clippy --workspace -- -D warnings` clean.
- [ ] No `unwrap()` on user-facing paths (the `Result::expect` rule
      from the in-band Dioxus 0.7 skill applies — use `?` and surface
      errors).
- [ ] No secrets in client-reachable code (`#[cfg(feature = "server")]`
      on anything that touches the filesystem or spawns processes).
- [ ] No new dependencies that aren't already in SPEC §4.1 (if you
      need one, justify it).
