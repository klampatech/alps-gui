//! Dashboard page (`/`).
//!
//! US-005 replaces the US-003 placeholder with the real layout:
//! a three-section responsive grid that composes the NewTask form,
//! the `FIXTURES` task list (one row per normal `TaskState` variant),
//! and a "Recent activity" placeholder. Everything reads from the
//! hardcoded fixture list — no live `use_resource` call yet (that
//! wiring lands in a follow-up story once the read-side server
//! functions ship in US-006/008).
//!
//! ## Layout (DESIGN.md §3 + §5)
//!
//! - Outer shell: `p-4 sm:p-6 lg:p-8 space-y-4` — DESIGN.md §2 page
//!   padding scale, with `space-y-4` between the page header and the
//!   responsive grid.
//! - `<h1>`: `text-2xl font-semibold text-slate-800` — DESIGN.md §4
//!   heading style.
//! - ResponsiveGrid: 1-col default / 3-col on `lg:` — three sections
//!   stacked on mobile, side-by-side on desktop.
//!
//! ## Sections (DESIGN.md §3 + US-005 acceptance #3)
//!
//! 1. NewTask form — `<form>` with a textarea for the prompt and a
//!    Submit button. The submit handler is a no-op for v1 (calls
//!    `prevent_default` so the page doesn't reload); the real
//!    `task_run` server-function call lands in US-006.
//! 2. Task list — `FIXTURES` rendered as cards. Each card shows a
//!    `StatusPill`, the `prompt_excerpt`, attempt count, and elapsed
//!    time. A `for` loop iterates over `FIXTURES.iter()` with a stable
//!    `key` derived from `task_id`.
//! 3. Recent activity — placeholder text "Recent log — coming in v2"
//!    inside the standard card chrome (DESIGN.md §2).
//!
//! ## Why `FIXTURES` is a `LazyLock` and not a `const` array
//!
//! `TaskSummary` has `String` fields; constructing a non-empty `String`
//! in a `const` context requires heap allocation which isn't stable in
//! Rust 1.83+. The fixtures module uses `LazyLock<Vec<TaskSummary>>`,
//! so the dashboard iterates via `FIXTURES.iter()` (each item is
//! `&TaskSummary`) and the slice returned by `&*FIXTURES` is
//! `&'static [TaskSummary]`. See `fixtures.rs` for the full rationale.
use dioxus::prelude::*;

use crate::components::{ResponsiveGrid, StatusPill};
use crate::fixtures::FIXTURES;

/// Format a duration in seconds as a short human-readable string.
///
/// Examples: `0 → "0s"`, `45 → "45s"`, `7320 → "2h 2m"`,
/// `525600 → "6d 6h"`. Used in the task list cards' "elapsed" cell.
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

/// Format a `DateTime<Utc>` as a short `MM-DD HH:MM` string for the
/// "created" cell. UTC is intentional — the orchestrator's clock and
/// the GUI's clock are both UTC, so a single explicit offset keeps the
/// fixture list readable without TZ confusion in the smoke run.
fn format_created_at(dt: chrono::DateTime<chrono::Utc>) -> String {
    dt.format("%m-%d %H:%M").to_string()
}

#[component]
pub fn Dashboard() -> Element {
    rsx! {
        div { class: "p-4 sm:p-6 lg:p-8 space-y-4",
            h1 { class: "text-2xl font-semibold text-slate-800", "Dashboard" }
            p { class: "text-sm text-slate-600",
                "Tasks shown below are the US-005 fixture list. A live list arrives when the read-side server function lands."
            }
            ResponsiveGrid {
                NewTaskSection {}
                TaskListSection {}
                RecentActivitySection {}
            }
        }
    }
}

/// (1) NewTask form — text area + Submit button.
///
/// US-005 acceptance #3 says this section's submit handler is a
/// no-op; we attach `prevent_default` to the submit event so the
/// browser doesn't reload the page. US-006 replaces the no-op with a
/// `task_run` server-function call.
#[component]
fn NewTaskSection() -> Element {
    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm space-y-3",
            h2 { class: "text-base font-medium text-slate-800", "New task" }
            p { class: "text-xs text-slate-500",
                "Describe what you want the orchestrator to do. Submit lands on the real `task_run` server function in US-006."
            }
            form {
                class: "space-y-2",
                // No-op v1: prevent the browser from reloading.
                onsubmit: move |evt| evt.prevent_default(),
                textarea {
                    class: "w-full rounded-md border border-slate-300 bg-white p-2 text-sm text-slate-800 focus:border-slate-500 focus:outline-none focus:ring-1 focus:ring-slate-500",
                    rows: "4",
                    placeholder: "e.g. Add a settings page so users can change the workdir without restarting the app.",
                }
                div { class: "flex justify-end",
                    button {
                        r#type: "submit",
                        class: "rounded-md bg-slate-800 px-3 py-1.5 text-sm font-medium text-white hover:bg-slate-700",
                        "Submit"
                    }
                }
            }
        }
    }
}

/// (2) Task list — one card per `FIXTURES` row.
///
/// Each card shows the `StatusPill`, the `prompt_excerpt` (truncated
/// to 200 chars — already enforced by the fixture module), the
/// attempt count, and elapsed time. The cards stack vertically in the
/// leftmost grid column on `lg:`.
#[component]
fn TaskListSection() -> Element {
    rsx! {
        div { class: "space-y-3 lg:col-span-2",
            h2 { class: "text-base font-medium text-slate-800", "Tasks" }
            for task in FIXTURES.iter() {
                // Stable per-task key — task_id is unique across fixtures.
                TaskCard { task: task.clone() }
            }
        }
    }
}

/// One task card: `StatusPill` + `prompt_excerpt` + meta row.
///
/// The card itself lives in this module rather than `components/`
/// because it's US-005-specific layout (no future page needs the same
/// shape — TaskDetail will use `StoryCard` / `ReceiptCard` /
/// `FindingCard` for its content, not a task-summary card).
#[component]
fn TaskCard(task: crate::domain::TaskSummary) -> Element {
    // Hoist format!() outputs OUTSIDE rsx! — Dioxus 0.7 rsx format-string
    // interpolation can't contain inner `{...}` braces (see US-004's
    // learning about the rsx parser tripping on `format!("{:?}", x)`).
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
///
/// Renders "Recent log — coming in v2" inside the canonical card chrome
/// per DESIGN.md §2. SSE-based log streaming lands in a follow-up story
/// (out of smoke scope per US-008 acceptance #3).
#[component]
fn RecentActivitySection() -> Element {
    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm space-y-2",
            h2 { class: "text-base font-medium text-slate-800", "Recent activity" }
            p { class: "text-sm text-slate-600", "Recent log — coming in v2" }
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

    #[test]
    fn dashboard_renders_eight_fixture_state_labels() {
        // Smoke test that the dashboard actually mounts the 8 FIXTURES
        // through `StatusPill`. The StatusPill unit tests cover each
        // individual pill; this test catches regressions where the
        // Dashboard's `for` loop is dropped or the FIXTURES list is
        // accidentally truncated.
        use crate::pages::Dashboard;

        let html = dioxus_ssr::render_element(rsx! {
            Dashboard {}
        });

        // All 8 normal state labels must appear in the rendered HTML.
        // (Unknown is intentionally absent from FIXTURES.)
        for label in [
            "Idle",
            "Planned",
            "Implemented",
            "Reviewed",
            "Running",
            "Done",
            "Rejected",
            "Failed",
        ] {
            let occurrences = html.matches(label).count();
            assert!(
                occurrences >= 1,
                "Dashboard HTML should contain at least one '{label}' label, but found {occurrences}",
            );
        }

        // Unknown should NOT appear as a fixture label — it only shows
        // up via the StatusPill unit test.
        assert!(
            !html.contains("Unknown"),
            "Dashboard HTML should NOT contain 'Unknown' (it's a corruption-only state)",
        );
    }
}
