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
//!   `resource.restart()` to re-fetch `tasks_list`).
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
//! ## Why a top-level `WORKDIR` constant (not yet a settings hook)
//!
//! M1 needs a workdir to pass to `tasks_list`. The right long-term
//! answer is the Settings page (M4) which surfaces the workdir as
//! editable + persisted to `~/.config/alps/ui.toml`. Until then, a
//! `pub const` here is the smallest workable surface — it lets the
//! Dashboard compile and exercise the live `tasks_list` path today,
//! and the Settings page (M4) will replace this constant with a
//! `Signal<String>` read from settings.
use dioxus::prelude::*;

use crate::api::{task_run, tasks_list};
use crate::components::{ResponsiveGrid, StatusPill};

/// Default workdir the Dashboard reads tasks from.
///
/// Override with the `ALPS_UI_WORKDIR` env var at serve time (the
/// `dx serve --` invocation picks it up via `std::env::var`). Once
/// M4 lands, this constant disappears and the workdir is read from
/// the persisted Settings (with this env-var as the cold-start
/// fallback).
fn default_workdir() -> String {
    std::env::var("ALPS_UI_WORKDIR")
        .unwrap_or_else(|_| format!("{}/Development/alps-runs", env!("HOME")))
}

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
    let workdir_signal = use_signal(default_workdir);
    let mut tasks_resource = use_resource(move || {
        let wd = workdir_signal.cloned();
        async move { tasks_list(wd).await }
    });

    // Re-fetch on Reload click. `use_resource::restart()` cancels the
    // current task and starts a new one.
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

    rsx! {
        div { class: "p-4 sm:p-6 lg:p-8 space-y-4",
            div { class: "flex items-baseline justify-between gap-4",
                h1 { class: "text-2xl font-semibold text-slate-800", "Dashboard" }
                button {
                    class: "rounded-md border border-slate-300 bg-white px-2.5 py-1 text-xs font-medium text-slate-700 hover:bg-slate-50 disabled:opacity-50",
                    title: "Reload tasks from {workdir_signal.cloned()}",
                    disabled: is_pending,
                    onclick: move |_| tasks_resource.restart(),
                    "↻ Reload"
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
        match task_run(default_workdir(), String::new(), prompt_text).await {
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

    rsx! {
        div {
            key: "{task.task_id}",
            class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm space-y-2",
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

        let html = dioxus_ssr::render_element(rsx! {
            Dashboard {}
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
