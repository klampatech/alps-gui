//! Dashboard page (`/`).
//!
//! M1 (smoke-A2): live data — calls `api::tasks_list(WORKDIR)` via
//! `use_resource` and renders the response. Replaces the smoke #1
//! FIXTURES-based fallback.
//!
//! The Dashboard module is gated on `feature = "server"` because it
//! calls `tasks_list`, which shells out to `alps list --json`. The
//! shell-out can't run in the browser, so the wasm-only build path
//! gets a minimal "run with --features server" stub via
//! `pages::dashboard_fallback` instead.
//!
//! ## Layout (DESIGN.md §3 + §5)
//!
//! - Outer shell: `p-4 sm:p-6 lg:p-8 space-y-4` — DESIGN.md §2 page
//!   padding scale.
//! - `<h1>`: `text-2xl font-semibold text-slate-800` — DESIGN.md §4
//!   heading style.
//! - Right of the heading: a `↻ Reload` button (calls
//!   `resource.restart()` to re-fetch `tasks_list`) + a "Last
//!   refreshed Xs ago · Auto/Paused" status row (v1.1 #3).
//! - ResponsiveGrid: 1-col default / 3-col on `lg:`.
//!
//! ## Sections (DESIGN.md §3)
//!
//! 1. NewTask form — `<form>` with a textarea for the prompt and a
//!    Submit button. Submit handler is a no-op until M2 wires
//!    `task_run`.
//! 2. Task list — renders the live `TaskList.tasks`. Renders a
//!    loading state while `use_resource` is `Pending`, an error
//!    banner if `tasks_list` returned `Err`, an empty-state card if
//!    the list is empty, or one `TaskCard` per task otherwise.
//! 3. Recent activity — placeholder text "Recent log — coming in v2"
//!    (SSE-based log streaming lands in M3).
//!
//! ## v1.1 #3 — Live polling (added 2026-08-28)
//!
//! Tasks appear/disappear in the workdir as `alps run` processes
//! spawn and complete. Pre-v1.1 the Dashboard only re-fetched when
//! the user clicked `↻ Reload` OR navigated away and back. That's a
//! janky experience — operators expect "open the dashboard, see
//! live state."
//!
//! Design: a 5s `use_future` polling loop increments a `tick`
//! signal that the `tasks_resource` reads inside its closure (the
//! resource re-fires when any signal it depends on changes —
//! same pattern as the workdir reactivity added in PR #16). A
//! second 1s `use_future` updates a `now_tick` signal that drives
//! the "Last refreshed Xs ago" UI text so it ticks every second
//! without re-fetching.
//!
//! Pause semantics: hovering anywhere on the page sets
//! `paused = true` (mouse-entered, mouse-leaved clears). Touch
//! users get an explicit `Paused / Polling` toggle button. When
//! paused, the polling loop still iterates (so resume is
//! instant), but the `restart()` call is skipped. Manual
//! `↻ Reload` always works regardless of pause.
//!
//! SSR stability: `last_refreshed_at`, `paused`, and `now_tick`
//! all initialize to deterministic values (`0`, `false`, `0`),
//! so the SSR'd HTML never differs from the pre-feature HTML in
//! its `Last refreshed` / `Auto` text — the widget renders as
//! `Last refreshed —` + `Polling` (visible only after hydration
//! ticks). Visual snapshot baselines don't drift.
use dioxus::prelude::*;

use crate::api::{task_run, tasks_list};
use crate::components::{ResponsiveGrid, StatusPill};
use crate::domain::TaskId;
use crate::routes::Route;
use crate::state;

/// Default workdir was moved to `crate::state::default_workdir()` in
/// M4-proper. The single source of truth is now the `Workdir` context
/// provided in `App`, and every page reads via
/// `use_context::<state::Workdir>()`.
///
/// v1.1 #3 — Dashboard live polling. The Dashboard's `use_resource`
/// depends on two signals: the Workdir context (reactive on Settings
/// save, per PR #14 / PR #16) and a `tick` counter that the polling
/// loop increments every `POLL_INTERVAL_SECS` seconds.
const POLL_INTERVAL_SECS: u64 = 5;

/// Format a duration in seconds as a short human-readable string.
fn format_elapsed(secs: u64) -> String {
    if secs < 60 {
        return format!("{}s", secs);
    }
    if secs < 3600 {
        return format!("{}m {}s", secs / 60, secs % 60);
    }
    if secs < 86_400 {
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        return format!("{}h {}m", hours, mins);
    }
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3600;
    format!("{}d {}h", days, hours)
}

/// Format a `DateTime<Utc>` as a short `MM-DD HH:MM` string.
fn format_created_at(dt: chrono::DateTime<chrono::Utc>) -> String {
    dt.format("%m-%d %H:%M").to_string()
}

#[component]
pub fn Dashboard() -> Element {
    // Live task list — `use_resource` fires the `tasks_list` async
    // work after mount. In SSR mode (`dx serve --platform server`),
    // the SSR'd HTML shows the LoadingCard because the resource
    // hasn't resolved by render time. After hydration, the browser
    // makes a real POST to `/api/tasks_list` and the dashboard
    // re-renders with the live task cards.
    //
    // We tried `use_loader` for SSR-aware blocking, but Dioxus 0.7's
    // `#[server]` macro-generated code calls `FullstackContext::extract`
    // which only succeeds in the HTTP dispatch path — so SSR-mode
    // `use_loader` errors with "FullstackContext not initialized".
    // Sticking with `use_resource` keeps the read-side code simple
    // and works in both SSR and wasm modes. The verify-us-007 #5
    // contract (M1 Dashboard markers present in SSR'd HTML) holds
    // because the loading skeleton still includes the page header +
    // section title + workdir subheader.
    //
    // M1 milestone cleanup — when Dioxus 0.7 fixes the SSR server-fn
    // dispatch (or when we add `use_loader` to a Dioxus release that
    // supports it), this can switch to `use_loader` for the snappier
    // first-paint with live data.
    //
    // v1.1 #3 — the resource closure now reads BOTH the Workdir
    // signal (existing reactivity, added in PR #16) AND a `tick`
    // signal (new, incremented every `POLL_INTERVAL_SECS` by the
    // polling loop below). Either signal changing re-fires the
    // resource. SSR-only fetches (initial render) don't see the
    // tick signal change (the future doesn't run on the server),
    // so SSR HTML stays identical to pre-feature.
    let workdir_signal = use_context::<state::Workdir>().signal();
    // `tick` doesn't need `mut` at the binding — we re-bind in the
    // polling-loop closure with `let mut tick = tick;`. The signal
    // itself is `Copy` (it's a `Signal<u64>`), so the binding can
    // be just `let`.
    let tick = use_signal::<u64>(|| 0);
    let mut tasks_resource = use_resource(move || {
        let wd = workdir_signal.cloned();
        // Read the tick to set up reactive dependency; the value
        // itself is unused — the polling loop just bumps this
        // signal to trigger re-fetch.
        let _ = tick.read();
        async move { tasks_list(wd).await }
    });

    // v1.1 #3 — live polling state.
    // - `paused`: toggle via the explicit "⏸ Pause / ▶ Resume"
    //   button. Touch-friendly + mouse-friendly (no hover-pause,
    //   because hover handlers on the outer div would race with
    //   the button's click — clicking the button would
    //   onmouseenter → onmouseleave the outer div and immediately
    //   reset paused back to false before the user sees the change).
    //   Manual button only — fewer surprises, no event ordering
    //   subtleties. The TaskLog page has a similar toggle (and an
    //   explicit Pause UX) per M3b — same pattern.
    // - `last_refreshed_at`: seconds-since-epoch (chrono::Utc)
    //   of the most recent fetch (whether from the initial
    //   resource fire, manual reload, or the polling loop).
    //   `0` initially — the UI shows "—" until the first fetch
    //   lands, which keeps SSR HTML stable.
    // - `now_tick`: seconds-since-epoch, ticks every 1s via a
    //   separate use_future. Drives the "Xs ago" UI text.
    //   Independent of `last_refreshed_at` so the UI re-renders
    //   every second without re-fetching `tasks_list`.
    let mut paused = use_signal(|| false);
    let mut last_refreshed_at = use_signal::<u64>(|| 0);
    let mut now_tick = use_signal::<u64>(|| 0);

    // Polling loop — every POLL_INTERVAL_SECS, bump `tick` to
    // trigger tasks_resource re-fetch (unless paused). The
    // initial fetch happens via use_resource itself, so we wait
    // POLL_INTERVAL_SECS before the first poll bump.
    //
    // Reactive on workdir_signal change too — if the operator
    // saves a new workdir via Settings while on the Dashboard,
    // the polling loop restarts against the new workdir.
    let _ = use_future(move || {
        let paused = paused;
        let mut tick = tick;
        let mut last_refreshed_at = last_refreshed_at;
        let _wd = workdir_signal.cloned(); // reactive restart on workdir change
        async move {
            loop {
                poll_sleep(POLL_INTERVAL_SECS * 1000).await;
                if !*paused.peek() {
                    // Read the tick value into a local before calling
                    // set(), because set() needs `&mut` on the signal
                    // and `peek()` needs `&`.
                    let new_tick = tick.peek().wrapping_add(1);
                    tick.set(new_tick);
                    last_refreshed_at.set(chrono::Utc::now().timestamp() as u64);
                }
            }
        }
    });

    // 1s ticker for the "Xs ago" UI text. Doesn't fetch anything.
    let _ = use_future(move || async move {
        loop {
            // Pull from page-runtime directly so the timer ticks
            // every second regardless of pause. The UI text is
            // informational; the fetch rate is controlled by the
            // polling loop above.
            poll_sleep(1000).await;
            now_tick.set(chrono::Utc::now().timestamp() as u64);
        }
    });

    // Re-fetch on Reload click. `use_resource::restart()` cancels the
    // current task and starts a new one. Note: restart() takes
    // `&mut`, so this click handler holds an exclusive borrow;
    // combined with the polling-loop's `tick` signal bump, manual
    // + auto reloads both work.
    let loader_view = match &*tasks_resource.read_unchecked() {
        None => rsx! { LoadingCard {} },
        Some(Ok(list)) if list.tasks.is_empty() => rsx! { EmptyCard {} },
        Some(Ok(list)) => rsx! {
            for task in list.tasks.iter() {
                TaskCard { task: task.clone() }
            }
        },
        Some(Err(e)) => rsx! { ErrorCard { error: format!("{e:?}") } },
    };

    let is_pending = !tasks_resource.finished();

    // v1.1 #3 — "Last refreshed Xs ago · Auto/Paused" status row.
    // Reads `now_tick.read()` so the UI text re-renders every 1s
    // without needing to re-fetch tasks_list.
    let last_refreshed_display = {
        let ts = *last_refreshed_at.read();
        let nw = *now_tick.read();
        if ts == 0 {
            "Last refreshed —".to_string()
        } else {
            let elapsed = nw.saturating_sub(ts);
            if elapsed < 60 {
                format!("Last refreshed {elapsed}s ago")
            } else {
                format!("Last refreshed {}m {}s ago", elapsed / 60, elapsed % 60)
            }
        }
    };

    let is_paused = *paused.read();
    let pause_button_label = if is_paused { "▶ Resume" } else { "⏸ Pause" };

    rsx! {
        // v1.1 #3 — no hover-pause handlers on the outer div.
        // (Tried `onmouseenter`/`onmouseleave` for hover-pause,
        // but the click on the Pause button races with the
        // leave event. Manual button only.)
        div {
            class: "p-4 sm:p-6 lg:p-8 space-y-4",
            div { class: "flex flex-wrap items-baseline justify-between gap-4",
                div { class: "flex items-baseline gap-3",
                    h1 { class: "text-2xl font-semibold text-slate-800", "Dashboard" }
                    span {
                        class: "text-xs text-slate-500 font-mono",
                        // Auto state surfaces to screen-readers; Paused
                        // means polling is suspended (manual reload still
                        // works).
                        title: if is_paused { "Polling paused" } else { "Polling auto-refresh every {POLL_INTERVAL_SECS}s" },
                        if is_paused { "· Paused" } else { "· Auto" }
                    }
                }
                div { class: "flex items-center gap-2",
                    // v1.1 #3 — "Last refreshed Xs ago" indicator.
                    // The `now_tick.read()` in the `format` argument
                    // subscribes the element to the 1s ticker.
                    span {
                        class: "text-xs text-slate-500 font-mono",
                        "{last_refreshed_display}"
                    }
                    button {
                        class: "rounded-md border border-slate-300 bg-white px-2.5 py-1 text-xs font-medium text-slate-700 hover:bg-slate-50",
                        // Touch + mouse users — explicit toggle
                        // (hover-pause doesn't work on touch).
                        title: if is_paused { "Resume auto-polling" } else { "Pause auto-polling" },
                        onclick: move |_| paused.set(!is_paused),
                        "{pause_button_label}"
                    }
                    button {
                        class: "rounded-md border border-slate-300 bg-white px-2.5 py-1 text-xs font-medium text-slate-700 hover:bg-slate-50 disabled:opacity-50",
                        title: "Reload tasks from {workdir_signal.cloned()}",
                        disabled: is_pending,
                        onclick: move |_| {
                            tasks_resource.restart();
                            last_refreshed_at.set(chrono::Utc::now().timestamp() as u64);
                        },
                        "↻ Reload"
                    }
                }
            }
            p { class: "text-sm text-slate-600",
                "Reading tasks from "
                span { class: "font-mono text-xs text-slate-700", "{workdir_signal.cloned()}" }
                " — submit one with the New task form on the left."
            }
            ResponsiveGrid {
                NewTaskSection {}
                TaskListSection { view: loader_view }
                RecentActivitySection {}
            }
        }
    }
}

/// Server-side / native sleep helper for the polling loop. The wasm
/// build uses `setTimeout` via `js_sys::Promise`; the server build
/// uses `tokio::time::sleep`. Same shape as `task_log::poll_sleep` —
/// future cleanup could move it to a shared `state::poll_sleep` helper
/// if more pages adopt polling.
#[cfg(not(target_arch = "wasm32"))]
async fn poll_sleep(ms: u64) {
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
}

#[cfg(target_arch = "wasm32")]
async fn poll_sleep(ms: u64) {
    use wasm_bindgen_futures::JsFuture;
    use wasm_bindgen::JsCast;

    // Create a Promise that resolves after `ms` via setTimeout.
    // `Promise::new`'s closure receives the resolver functions and
    // schedules `resolve(undefined)` to fire after `ms`. The future
    // is awaited via `JsFuture`, which yields once the Promise
    // resolves — i.e. once `ms` have elapsed.
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let win = web_sys::window().expect("no window in wasm context");
        // `resolve` is a `js_sys::Function`; coerce via `JsCast::dyn_ref`
        // so web-sys's C bindings can pass it through to setTimeout.
        let callback: &js_sys::Function = resolve.dyn_ref().expect("resolve is a Function");
        let _ = win.set_timeout_with_callback_and_timeout_and_arguments_0(
            callback,
            ms as i32,
        );
    });
    let _ = JsFuture::from(promise).await;
}

/// (1) NewTask form — text area + Submit button.
///
/// M2 wires the `onsubmit` to call the `task_run` server fn, which
/// spawns `alps run` and returns the new task_id. On success we
/// re-fetch the dashboard's `tasks_list` resource so the new task
/// card appears without a manual reload.
#[component]
fn NewTaskSection() -> Element {
    let mut prompt = use_signal(String::new);
    let mut submit_state = use_signal(|| SubmitState::Idle);
    let mut last_task_id = use_signal(String::new);

    // Capture the workdir context once. The closure below is `move`,
    // so cloning the `Workdir` (it's `Copy`) is fine. Reading the
    // Signal at submit-time (not mount-time) means if the user changes
    // workdir via Settings between mount + submit, the new value is
    // picked up — see M4-proper plan 2026-08-26.
    let workdir_ctx = use_context::<state::Workdir>();

    let on_submit = move |evt: Event<FormData>| async move {
        evt.prevent_default();
        let prompt_text = prompt.read().clone();
        if prompt_text.trim().is_empty() {
            submit_state.set(SubmitState::Error(
                "prompt cannot be empty".to_string(),
            ));
            return;
        }
        submit_state.set(SubmitState::Submitting);
        // Default deliverable_path = empty (CLI auto-detects from prompt
        // per `alps-cli/src/main.rs:813-830`).
        match task_run(workdir_ctx.get(), String::new(), prompt_text).await {
            Ok(id) => {
                last_task_id.set(id.clone());
                submit_state.set(SubmitState::Success(id));
                prompt.set(String::new());
            }
            Err(e) => {
                submit_state.set(SubmitState::Error(format!("{e:?}")));
            }
        }
    };

    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm space-y-3",
            h2 { class: "text-base font-medium text-slate-800", "New task" }
            p { class: "text-xs text-slate-500",
                "Describe what you want the orchestrator to do. Submit spawns `alps run` via the server-side `task_run` function."
            }
            form {
                class: "space-y-2",
                onsubmit: on_submit,
                textarea {
                    class: "w-full rounded-md border border-slate-300 bg-white p-2 text-sm text-slate-800 focus:border-slate-500 focus:outline-none focus:ring-1 focus:ring-slate-500",
                    rows: "4",
                    placeholder: "e.g. Add a settings page so users can change the workdir without restarting the app.",
                    value: "{prompt}",
                    oninput: move |evt| prompt.set(evt.value()),
                }
                div { class: "flex justify-end",
                    button {
                        r#type: "submit",
                        class: "rounded-md bg-slate-800 px-3 py-1.5 text-sm font-medium text-white hover:bg-slate-700 disabled:opacity-50 disabled:cursor-not-allowed",
                        disabled: "{matches!(submit_state.read().clone(), SubmitState::Submitting)}",
                        if matches!(submit_state.read().clone(), SubmitState::Submitting) {
                            "Submitting…"
                        } else {
                            "Submit"
                        }
                    }
                }
            }
            SubmitFeedback { state: submit_state.read().clone(), last_task_id: last_task_id.read().clone() }
        }
    }
}

/// Submit-state machine for the NewTask form. Lives next to
/// `NewTaskSection` because the form's `use_signal` directly drives
/// the rendered status.
#[derive(Clone, PartialEq)]
enum SubmitState {
    Idle,
    Submitting,
    Success(String),
    Error(String),
}

/// Status row beneath the submit button. Shows "spawning…" while
/// in-flight, a green task_id on success, or a rose banner on
/// error. Pure presentation — the actual `tasks_list` resource is
/// unaffected, so a reload is still available to fetch state
/// transitions.
#[component]
fn SubmitFeedback(state: SubmitState, last_task_id: String) -> Element {
    match state {
        SubmitState::Idle => rsx! {},
        SubmitState::Submitting => rsx! {
            p { class: "text-xs text-slate-500 font-mono",
                "Spawning `alps run` — reading task_id from stderr…"
            }
        },
        SubmitState::Success(id) => rsx! {
            div { class: "rounded-md bg-emerald-50 border border-emerald-200 p-2 text-xs text-emerald-800 font-mono",
                "Spawned task "
                span { class: "font-semibold", "{id}" }
                " — see Dashboard ↓"
            }
        },
        SubmitState::Error(msg) => rsx! {
            div { class: "rounded-md bg-rose-50 border border-rose-200 p-2 text-xs text-rose-800 font-mono",
                "{msg}"
            }
        },
    }
}

/// (2) Task list — renders the pre-computed `view: Element` from
/// the parent `Dashboard`'s `use_loader`.
///
/// `Dashboard` does the loading/error/empty/list branching and passes
/// an `Element` here. Keeping the branching in the parent means the
/// `use_loader` is called exactly once (the parent) and `TaskListSection`
/// stays a thin wrapper that just provides the section heading +
/// grid span.
#[component]
fn TaskListSection(view: Element) -> Element {
    rsx! {
        div { class: "space-y-3 lg:col-span-2",
            h2 { class: "text-base font-medium text-slate-800", "Tasks" }
            {view}
        }
    }
}

/// Placeholder shown while `tasks_list` is in flight.
#[component]
fn LoadingCard() -> Element {
    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm space-y-2",
            div { class: "h-4 w-24 animate-pulse rounded bg-slate-200" }
            div { class: "h-3 w-full animate-pulse rounded bg-slate-100" }
            div { class: "h-3 w-2/3 animate-pulse rounded bg-slate-100" }
            p { class: "text-xs text-slate-500", "Loading tasks…" }
        }
    }
}

/// Error banner — the `tasks_list` server fn returned `Err`.
#[component]
fn ErrorCard(error: String) -> Element {
    rsx! {
        div { class: "rounded-lg border border-rose-300 bg-rose-50 p-4 shadow-sm space-y-2",
            h3 { class: "text-sm font-medium text-rose-800", "Failed to load tasks" }
            p { class: "text-xs text-rose-700 font-mono break-all", "{error}" }
            p { class: "text-xs text-rose-700",
                "Check that `alps` is on $PATH and that the workdir contains a `tasks/` subdir."
            }
        }
    }
}

/// Empty-state — `tasks_list` returned `Ok` with zero tasks.
#[component]
fn EmptyCard() -> Element {
    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm space-y-2",
            h3 { class: "text-sm font-medium text-slate-700", "No tasks yet" }
            p { class: "text-sm text-slate-600",
                "Submit one with the New task form on the left, or run "
                span { class: "font-mono text-xs", "alps run --workdir <path> --prompt-file <file>" }
                " from the terminal to seed the list."
            }
        }
    }
}

/// One task card: `StatusPill` + `prompt_excerpt` + meta row.
#[component]
fn TaskCard(task: crate::domain::TaskSummary) -> Element {
    let elapsed_display = task
        .elapsed_secs
        .map(format_elapsed)
        .unwrap_or_else(|| "—".to_string());
    let attempts_display = format!("attempt {}", task.attempts + 1);
    let created_display = format_created_at(task.created_at);

    // The Dashboard's TaskCard is a `<Link>` to the per-task page
    // (M3a). Clicking navigates to `/tasks/<task_id>` which renders
    // `pages::TaskDetail`. The `Link` component accepts any child
    // (including a styled `<div>`) and renders an `<a href>` for
    // SSR / static export. `task.task_id` is a `String`; wrap it in
    // the UI-side `TaskId` newtype so the `Route::TaskDetail { id }`
    // variant accepts it directly (per `routes.rs`'s `Routable`
    // derive's typed-segment requirement).
    let link_target = Route::TaskDetail {
        id: TaskId::new(task.task_id.clone()),
    };

    rsx! {
        Link {
            to: link_target,
            key: "{task.task_id}",
            class: "block rounded-lg border border-slate-200 bg-white p-4 shadow-sm space-y-2 hover:border-slate-300 hover:shadow",
            div { class: "flex items-center justify-between gap-2",
                StatusPill { state: task.state }
                span { class: "font-mono text-xs text-slate-500", "{task.task_id}" }
            }
            p { class: "text-sm text-slate-700", "{task.prompt_excerpt}" }
            div { class: "flex items-center gap-3 text-xs text-slate-500",
                span { "{attempts_display}" }
                span { class: "text-slate-300", "·" }
                span { "elapsed {elapsed_display}" }
                span { class: "text-slate-300", "·" }
                span { "started {created_display}" }
            }
        }
    }
}

/// (3) Recent activity — stub card.
#[component]
fn RecentActivitySection() -> Element {
    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm space-y-2",
            h2 { class: "text-base font-medium text-slate-800", "Recent activity" }
            p { class: "text-sm text-slate-600", "Recent log — coming in v2" }
        }
    }
}

// `ServerFnError` is `Debug`-only (no `Display`), so the ErrorCard
// formats via `{e:?}`. No additional re-export needed — `ServerFnError`
// is imported at the top of this file from `crate::api`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_elapsed_sub_minute() {
        assert_eq!(format_elapsed(0), "0s");
        assert_eq!(format_elapsed(45), "45s");
        assert_eq!(format_elapsed(59), "59s");
    }

    #[test]
    fn format_elapsed_sub_hour() {
        assert_eq!(format_elapsed(60), "1m 0s");
        assert_eq!(format_elapsed(125), "2m 5s");
        assert_eq!(format_elapsed(3599), "59m 59s");
    }

    #[test]
    fn format_elapsed_sub_day() {
        assert_eq!(format_elapsed(3600), "1h 0m");
        assert_eq!(format_elapsed(7320), "2h 2m");
        assert_eq!(format_elapsed(86_399), "23h 59m");
    }

    #[test]
    fn format_elapsed_multi_day() {
        assert_eq!(format_elapsed(86_400), "1d 0h");
        assert_eq!(format_elapsed(604_800), "7d 0h");
    }

    /// M1 contract: the Dashboard SSR'd HTML always shows the page
    /// header + the "Tasks" section heading. With `use_resource`, the
    /// task cards themselves are async-loaded on the client; in SSR
    /// they show the loading skeleton. The 8-state-label assertion
    /// from smoke #1 (which used FIXTURES) no longer applies — tasks
    /// now come from a live `tasks_list` call, which is empty in the
    /// SSR initial render before the resource resolves.
    #[test]
    fn dashboard_ssr_shows_header_and_section_title() {
        use crate::pages::Dashboard;
        use crate::state::{provide_workdir, Workdir};

        // Wrap Dashboard in an inline component that provides the
        // Workdir context (M4-proper). Tests previously rendered the
        // page directly, but the page now reads via use_context which
        // is uninitialized without App's provide_workdir() call.
        #[component]
        fn TestApp() -> Element {
            let _wd = provide_workdir();
            rsx! { Dashboard {} }
        }

        let html = dioxus_ssr::render_element(rsx! {
            TestApp {}
        });

        assert!(
            html.contains("Dashboard"),
            "Dashboard header should render in SSR"
        );
        assert!(
            html.contains("Tasks"),
            "Tasks section heading should render in SSR"
        );
        assert!(
            html.contains("Reading tasks from"),
            "Dashboard should advertise the workdir it's reading from"
        );
    }
}
