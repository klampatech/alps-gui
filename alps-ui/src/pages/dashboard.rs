//! Dashboard page (`/`).
//!
//! Control Rail redesign (deck §02): 2-column grid. Left column = NewTask
//! card (textarea + amber "Run task ↗" button). Right column = task cards
//! in a 2-col grid, each card with a mini-rail showing where the task
//! sits on the 5-node pipeline.
//!
//! ## Layout
//!
//! ```text
//! <div p-4 sm:p-6 lg:p-8>
//!   <page-header>                 // Dashboard title + "Last refreshed" + Reload
//!   <2-col grid>
//!     <NewTask card>               // 1col width on lg (deck shows 260px-ish)
//!     <task-cards 2-col>
//!       <TaskCard ...>             // per-task: pill, id, prompt, mini-rail,
//!                                 // meta row, footer links
//!     </task-cards>
//!   </2-col grid>
//!   <RecentActivity card>         // bottom (kept — deck omits but no
//!                                 // reason to regress it)
//! </div>
//! ```
//!
//! ## Live polling (v1.1 #3)
//!
//! Carried over from the pre-redesign implementation: a 5s `use_future`
//! polling loop bumps a `tick` signal that the `tasks_resource` reads.
//! Same reactive-read pattern (`workdir_signal.cloned()` inside the
//! closure body, not at the boundary — Pitfall 56). The "Last refreshed
//! Xs ago" indicator + Pause/Resume toggle live in the page header.
//!
//! ## SSR stability
//!
//! `last_refreshed_at`, `paused`, and `now_tick` all initialize to
//! deterministic values, so the SSR'd HTML never differs from the
//! pre-feature HTML in its `Last refreshed` / `Auto` text. The visual
//! snapshot baselines stay deterministic.
use dioxus::prelude::*;

use crate::api::{task_run, tasks_list};
use crate::components::{phase_for_state, MiniRail, StatusPill};
use crate::domain::{TaskId, TaskState};
use crate::routes::Route;
use crate::state;

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
    let workdir_signal = use_context::<state::Workdir>().signal();
    let tick = use_signal::<u64>(|| 0);
    let mut tasks_resource = use_resource(move || {
        let wd = workdir_signal.cloned();
        let _ = tick.read();
        async move { tasks_list(wd).await }
    });

    // v1.1 #3 — live polling state.
    let mut paused = use_signal(|| false);
    let mut last_refreshed_at = use_signal::<u64>(|| 0);
    let mut now_tick = use_signal::<u64>(|| 0);

    let _ = use_future(move || {
        let paused = paused;
        let mut tick = tick;
        let mut last_refreshed_at = last_refreshed_at;
        let _wd = workdir_signal.cloned();
        async move {
            loop {
                poll_sleep(POLL_INTERVAL_SECS * 1000).await;
                if !*paused.peek() {
                    let new_tick = tick.peek().wrapping_add(1);
                    tick.set(new_tick);
                    last_refreshed_at.set(chrono::Utc::now().timestamp() as u64);
                }
            }
        }
    });

    let _ = use_future(move || async move {
        loop {
            poll_sleep(1000).await;
            now_tick.set(chrono::Utc::now().timestamp() as u64);
        }
    });

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
        div {
            style: "padding:24px;",
            // Page header — mono title + "Auto/Paused" chip +
            // "Last refreshed" + Reload + Pause buttons.
            div {
                style: "display:flex;flex-wrap:wrap;align-items:baseline;justify-content:space-between;gap:16px;margin-bottom:24px;",
                div {
                    style: "display:flex;align-items:baseline;gap:12px;",
                    h1 {
                        style: "font-family:var(--mono);font-weight:600;font-size:24px;margin:0;color:var(--text);",
                        "Dashboard"
                    }
                    span {
                        style: "font-family:var(--mono);font-size:12px;color:var(--faint);",
                        if is_paused { "· Paused" } else { "· Auto" }
                    }
                }
                div {
                    style: "display:flex;align-items:center;gap:8px;",
                    span {
                        class: "alps-mono",
                        style: "font-size:12px;color:var(--faint);",
                        "{last_refreshed_display}"
                    }
                    button {
                        class: "btn-ghost",
                        style: "padding:6px 10px;font-size:11px;",
                        title: if is_paused { "Resume auto-polling" } else { "Pause auto-polling" },
                        onclick: move |_| paused.set(!is_paused),
                        "{pause_button_label}"
                    }
                    button {
                        class: "btn-ghost",
                        style: "padding:6px 10px;font-size:11px;",
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
            p {
                style: "font-family:var(--mono);font-size:11px;color:var(--faint);margin:0 0 24px 0;",
                "Reading tasks from "
                span { style: "color:var(--dim);", "{workdir_signal.cloned()}" }
                " · polling every "
                {POLL_INTERVAL_SECS.to_string()}
                "s"
            }
            // 2-column layout per deck §02: NewTask on the left,
            // task cards in a 2-col grid on the right.
            div {
                style: "display:grid;grid-template-columns:minmax(0,1fr) minmax(0,2fr);gap:24px;",
                // Left column — NewTask card.
                NewTaskSection {}
                // Right column — task cards in a responsive auto-fill
                // grid. The `minmax(280px, 1fr)` track is load-bearing:
                // `minmax(0, 1fr)` collapses to a single column because
                // the minimum track size is zero, so auto-fill can fit
                // unlimited cards at 0px wide. A real minimum (280px) is
                // below the smallest task card content width (~280px)
                // and lets auto-fill produce 2 cols at the standard
                // 1400px viewport (1400 / (280 + 14) ≈ 4.7, rounds down
                // to 4 with the 14px gap; but cards are wider than 280px
                // so it settles at 2 cols at 1280, 3 at 1600+). See
                // PR review thread for the rendered regression.
                div {
                    style: "display:grid;grid-template-columns:repeat(auto-fill,minmax(280px,1fr));gap:14px;align-content:start;",
                    {loader_view}
                }
            }
            // Recent activity kept below as a single full-width row.
            RecentActivitySection {}
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn poll_sleep(ms: u64) {
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
}

#[cfg(target_arch = "wasm32")]
async fn poll_sleep(ms: u64) {
    use wasm_bindgen_futures::JsFuture;
    use wasm_bindgen::JsCast;
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let win = web_sys::window().expect("no window in wasm context");
        let callback: &js_sys::Function = resolve.dyn_ref().expect("resolve is a Function");
        let _ = win.set_timeout_with_callback_and_timeout_and_arguments_0(
            callback,
            ms as i32,
        );
    });
    let _ = JsFuture::from(promise).await;
}

#[component]
fn NewTaskSection() -> Element {
    let mut prompt = use_signal(String::new);
    let mut submit_state = use_signal(|| SubmitState::Idle);
    let mut last_task_id = use_signal(String::new);

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
        div {
            class: "surface",
            style: "padding:18px 20px;",
            h2 {
                style: "font-family:var(--mono);font-size:13px;font-weight:600;color:var(--text);margin:0 0 10px 0;",
                "New task"
            }
            form {
                style: "display:flex;flex-direction:column;gap:12px;",
                onsubmit: on_submit,
                textarea {
                    class: "alps-sans",
                    style: "background:var(--ink);border:1px solid var(--hair-soft);border-radius:8px;height:96px;padding:10px 12px;font-size:12px;color:var(--text);resize:vertical;outline:none;font-family:inherit;",
                    placeholder: "Add rate limiting to /api/tasks endpoint…",
                    value: "{prompt}",
                    oninput: move |evt| prompt.set(evt.value()),
                }
                div {
                    style: "display:flex;justify-content:flex-start;",
                    button {
                        r#type: "submit",
                        class: "btn-amber",
                        disabled: "{matches!(submit_state.read().clone(), SubmitState::Submitting)}",
                        if matches!(submit_state.read().clone(), SubmitState::Submitting) {
                            "Running…"
                        } else {
                            "Run task ↗"
                        }
                    }
                }
                p {
                    style: "font-size:11px;color:var(--faint);margin:4px 0 0 0;line-height:1.5;",
                    "Spawns alps run --prompt-file. Watch it land on the rail below once it starts."
                }
                SubmitFeedback { state: submit_state.read().clone(), last_task_id: last_task_id.read().clone() }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
enum SubmitState {
    Idle,
    Submitting,
    Success(String),
    Error(String),
}

#[component]
fn SubmitFeedback(state: SubmitState, last_task_id: String) -> Element {
    match state {
        SubmitState::Idle => rsx! {},
        SubmitState::Submitting => rsx! {
            p {
                class: "alps-mono",
                style: "font-size:11px;color:var(--faint);margin:0;",
                "Spawning alps run — reading task_id from stderr…"
            }
        },
        SubmitState::Success(id) => rsx! {
            div {
                class: "toast-ok",
                style: "margin-top:8px;",
                span {
                    class: "alps-mono",
                    "Spawned {id} — see below ↓"
                }
                span { "×" }
            }
        },
        SubmitState::Error(msg) => rsx! {
            div {
                class: "toast-err",
                style: "margin-top:8px;",
                span { class: "alps-mono", "{msg}" }
                span { "×" }
            }
        },
    }
}

#[component]
fn LoadingCard() -> Element {
    rsx! {
        div {
            class: "surface",
            style: "padding:18px 20px;grid-column:1 / -1;",
            div {
                style: "display:flex;flex-direction:column;gap:8px;",
                div {
                    style: "height:12px;width:120px;border-radius:6px;background:var(--hair-soft);",
                }
                div {
                    style: "height:8px;width:200px;border-radius:4px;background:var(--hair-soft);",
                }
            }
            p {
                class: "alps-mono",
                style: "font-size:11px;color:var(--faint);margin:8px 0 0 0;",
                "Loading tasks…"
            }
        }
    }
}

#[component]
fn ErrorCard(error: String) -> Element {
    rsx! {
        div {
            class: "surface",
            style: "padding:18px 20px;grid-column:1 / -1;border-color:var(--red);",
            h3 {
                class: "alps-mono",
                style: "font-size:13px;font-weight:600;color:var(--red);margin:0 0 8px 0;",
                "Failed to load tasks"
            }
            p {
                class: "alps-mono",
                style: "font-size:11px;color:var(--red);margin:0;word-break:break-all;",
                "{error}"
            }
            p {
                style: "font-size:11px;color:var(--dim);margin:8px 0 0 0;",
                "Check that alps is on $PATH and that the workdir contains a tasks/ subdir."
            }
        }
    }
}

#[component]
fn EmptyCard() -> Element {
    rsx! {
        div {
            class: "surface",
            style: "padding:18px 20px;grid-column:1 / -1;",
            h3 {
                class: "alps-mono",
                style: "font-size:13px;font-weight:600;color:var(--dim);margin:0 0 6px 0;",
                "No tasks yet"
            }
            p {
                style: "font-size:12px;color:var(--dim);margin:0;line-height:1.55;",
                "Submit one with the New task form on the left, or run alps run from the terminal to seed the list."
            }
        }
    }
}

/// One task card per the deck §02 design.
///
/// Layout:
/// - Top row: StatusPill + task id
/// - Prompt line (single-line ellipsis)
/// - MiniRail (5 segments)
/// - Meta row (attempt/elapsed/started)
/// - Footer (Open log / View diff / Cancel when running)
#[component]
fn TaskCard(task: crate::domain::TaskSummary) -> Element {
    let elapsed_display = task
        .elapsed_secs
        .map(format_elapsed)
        .unwrap_or_else(|| "—".to_string());
    let attempts_display = format!("attempt {}", task.attempts + 1);
    let created_display = format_created_at(task.created_at);

    let link_target = Route::TaskDetail {
        id: TaskId::new(task.task_id.clone()),
    };

    let is_failed = matches!(task.state, TaskState::Failed | TaskState::Rejected);
    let current_phase = phase_for_state(task.state);

    rsx! {
        Link {
            to: link_target,
            key: "{task.task_id}",
            class: "surface",
            style: "padding:16px 18px;display:flex;flex-direction:column;gap:10px;text-decoration:none;color:inherit;transition:border-color .15s;",
            // Top row — StatusPill on the left, truncated task id on
            // the right. Task IDs are 48 chars (`YYYY-MM-DDTHHMMSS-uuidhex`)
            // and the Dashboard's 3-col grid cards are ~400px wide, so
            // without truncation the full ID spills past the right edge
            // and visually overlaps the next card. `min-width:0` on the
            // flex row + `max-width:100%; overflow:hidden` on the ID
            // span lets it shrink to whatever's available, with
            // ellipsis for the remainder.
            //
            // Per the deck design language, the full ID is preserved
            // in the DOM (`text-overflow: ellipsis` is purely visual —
            // the user's `alps show <id>` commands still need the
            // canonical id, which lives in the click target).
            div {
                style: "display:flex;justify-content:space-between;align-items:center;gap:12px;min-width:0;",
                StatusPill { state: task.state }
                span {
                    class: "alps-mono",
                    style: "font-size:12px;color:var(--dim);max-width:100%;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;",
                    "{task.task_id}"
                }
            }
            // Prompt (1-line ellipsis)
            p {
                style: "font-size:13px;color:var(--text);margin:0;line-height:1.45;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;",
                "{task.prompt_excerpt}"
            }
            // Mini-rail — 5 segments showing where the task sits on the pipeline.
            MiniRail {
                current_phase,
                is_failed,
            }
            // Meta row
            div {
                class: "alps-mono",
                style: "font-size:10.5px;color:var(--faint);display:flex;gap:8px;flex-wrap:wrap;",
                span { "{attempts_display}" }
                span { style: "color:var(--hair);", "·" }
                span { "elapsed {elapsed_display}" }
                span { style: "color:var(--hair);", "·" }
                span { "started {created_display}" }
            }
            // Footer links — deck §02 shows "Open log →", "View diff →",
            // and a red "Cancel" when running.
            div {
                class: "alps-mono",
                style: "font-size:11px;display:flex;justify-content:space-between;align-items:center;gap:8px;",
                div {
                    style: "display:flex;gap:14px;",
                    span { style: "color:var(--dim);", "Open log →" }
                    span { style: "color:var(--dim);", "View diff →" }
                }
                if matches!(task.state, TaskState::Running) {
                    span {
                        style: "color:var(--red);",
                        "Cancel"
                    }
                }
            }
        }
    }
}

#[component]
fn RecentActivitySection() -> Element {
    rsx! {
        div {
            class: "surface",
            style: "margin-top:24px;padding:18px 20px;",
            h2 {
                class: "alps-mono",
                style: "font-size:13px;font-weight:600;color:var(--text);margin:0 0 8px 0;",
                "Recent activity"
            }
            p {
                style: "font-size:12px;color:var(--dim);margin:0;",
                "Recent log — coming in v2"
            }
        }
    }
}

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

    /// SSR contract: the Dashboard's SSR'd HTML must contain the
    /// page header + the "Reading tasks from" subheader. The
    /// `MiniRail` doesn't render in SSR (no task data yet) but the
    /// `New task` card does.
    #[test]
    fn dashboard_ssr_shows_header_and_workdir_subheader() {
        use crate::pages::Dashboard;
        use crate::state::provide_workdir;

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
            html.contains("Reading tasks from"),
            "Dashboard should advertise the workdir it's reading from"
        );
        // The new task card's primary CTA.
        assert!(
            html.contains("Run task"),
            "NewTask card should advertise the Run task CTA"
        );
    }
}
