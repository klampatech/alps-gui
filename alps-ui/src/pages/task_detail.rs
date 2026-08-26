//! TaskDetail page (`/tasks/:id`).
//!
//! M3a: per-task page that calls `api::task_get(workdir, id)` via
//! `use_resource` and renders the returned `TaskDetail` using the
//! existing `StatusPill`, `StoryCard`, `FindingCard`, `AssertionCard`,
//! and `ReceiptCard` components (US-004).
//!
//! ## What renders
//!
//! - **Header**: `StatusPill` (state from `detail.summary.state`),
//!   prompt excerpt from `detail.summary.prompt_excerpt`, and a meta
//!   row with task_id, branch (from `detail.implementation.ralph_branch`
//!   if present), attempts, created_at, elapsed.
//! - **Plan section**: one `StoryCard` per `detail.plan.stories`. If
//!   `detail.plan` is `None`, render a placeholder.
//! - **Review section**: one `FindingCard` per `detail.review.findings`,
//!   then one `AssertionCard` per `detail.review.assertions`. If
//!   `detail.review` is `None`, render a placeholder.
//! - **Receipts section**: one `ReceiptCard` from `detail.receipts`.
//!   If `None`, render a placeholder.
//!
//! ## Loading / error / not-found branches
//!
//! - `use_resource` `Pending` → `LoadingCard`
//! - `use_resource` `Some(Ok(None))` → 404 card (the CLI exited 2)
//! - `use_resource` `Some(Err(e))` → error banner
//! - `use_resource` `Some(Ok(Some(detail)))` → populated render
//!
//! ## Why no `#[cfg(feature = "server")]` on the page module
//!
//! The Dashboard (`pages::dashboard`) compiles cleanly in both the
//! wasm and server builds because the `api` module exports a wasm
//! stub for every server fn (see `api/mod.rs:118-205`). TaskDetail
//! follows the same pattern: no module-level gate, just a direct call
//! to `task_get` which resolves to the server fn or the wasm stub
//! depending on the build target. The wasm build gets a populated
//! page that dispatches via web-sys fetch; the SSR build gets the
//! same dispatch via axum.

use dioxus::prelude::*;

use crate::api::task_get;
use crate::components::{AssertionCard, FindingCard, ReceiptCard, StatusPill, StoryCard};
use crate::domain::TaskId;
use crate::routes::Route;

/// Read the default workdir from `ALPS_UI_WORKDIR` or fall back to
/// `~/Development/alps-runs`. Same logic as Dashboard's
/// `default_workdir` — kept inline here to avoid a cross-module
/// coupling between `pages::dashboard` and `pages::task_detail`.
fn default_workdir() -> String {
    std::env::var("ALPS_UI_WORKDIR")
        .unwrap_or_else(|_| format!("{}/Development/alps-runs", env!("HOME")))
}

/// Format a duration in seconds as a short human-readable string.
/// Mirrors Dashboard's helper so the two pages render elapsed time
/// identically.
fn format_elapsed(secs: u64) -> String {
    if secs < 60 {
        return format!("{}s", secs);
    }
    if secs < 3600 {
        return format!("{}m {}s", secs / 60, secs % 60);
    }
    if secs < 86_400 {
        return format!("{}h {}m", secs / 3600, (secs % 3600) / 60);
    }
    format!("{}d {}h", secs / 86_400, (secs % 86_400) / 3600)
}

/// `DateTime<Utc>` → "MM-DD HH:MM" for the meta row.
fn format_created_at(dt: chrono::DateTime<chrono::Utc>) -> String {
    dt.format("%m-%d %H:%M").to_string()
}

#[component]
pub fn TaskDetail(id: TaskId) -> Element {
    // The route already gives us a typed `TaskId`; pass `id.0.clone()`
    // into the server fn because the async closure needs an owned
    // String (the closure outlives the call frame).
    let workdir = default_workdir();
    let task_id_for_fn = id.0.clone();
    let task_id_for_display = id.0.clone();
    let resource = use_resource(move || {
        let wd = workdir.clone();
        let tid = task_id_for_fn.clone();
        async move { task_get(wd, tid).await }
    });

    let body = match &*resource.read_unchecked() {
        None => rsx! { LoadingCard {} },
        Some(Ok(None)) => rsx! {
            NotFoundCard { id: id.0.clone(), workdir: default_workdir() }
        },
        Some(Ok(Some(detail))) => rsx! {
            PopulatedDetail {
                detail: detail.clone(),
                task_id: task_id_for_display.clone(),
                id: id.clone(),
            }
        },
        Some(Err(e)) => rsx! {
            ErrorCard { error: format!("{e:?}") }
        },
    };

    rsx! {
        div { class: "p-4 sm:p-6 lg:p-8 space-y-4",
            h1 { class: "text-2xl font-semibold text-slate-800",
                "Task "
                span { class: "font-mono text-base text-slate-500", "{task_id_for_display}" }
            }
            {body}
        }
    }
}

/// The populated render — only reached when `task_get` returned
/// `Ok(Some(TaskDetail))`. Sub-rendered into Plan / Review / Receipts
/// sections, each of which can be missing (a None on the
/// corresponding TaskDetail field renders a placeholder card).
#[component]
fn PopulatedDetail(
    detail: crate::domain::TaskDetail,
    task_id: String,
    id: TaskId,
) -> Element {
    let summary = detail.summary.clone();
    let elapsed_display = summary
        .elapsed_secs
        .map(format_elapsed)
        .unwrap_or_else(|| "—".to_string());
    let attempts_display = format!("attempt {}", summary.attempts + 1);
    let created_display = format_created_at(summary.created_at);
    // Branch name lives in Implementation (when implemented), not on the
    // summary. Compute once, render conditionally.
    let branch_display = detail
        .implementation
        .as_ref()
        .map(|impl_| impl_.ralph_branch.clone())
        .unwrap_or_else(|| "—".to_string());

    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm space-y-3",
            div { class: "flex items-center justify-between gap-2",
                StatusPill { state: summary.state }
                span { class: "font-mono text-xs text-slate-500", "{task_id}" }
            }
            p { class: "text-sm text-slate-700", "{summary.prompt_excerpt}" }
            div { class: "flex flex-wrap items-center gap-3 text-xs text-slate-500",
                span { "{attempts_display}" }
                span { class: "text-slate-300", "·" }
                span { "elapsed {elapsed_display}" }
                span { class: "text-slate-300", "·" }
                span { "started {created_display}" }
                span { class: "text-slate-300", "·" }
                span { "branch {branch_display}" }
            }
        }

        PlanSection { plan: detail.plan.clone() }

        ReviewSection { review: detail.review.clone() }

        ReceiptsSection { receipts: detail.receipts.clone() }

        // Footer: navigation links to Log / Diff / Cancel. M3b wires the
// "Open log" stub to a real <Link> pointing at Route::TaskLog. M3c
// will replace "View diff" and "Cancel" with the same pattern.
        div { class: "flex items-center gap-3 text-sm",
            Link {
                to: Route::TaskLog { id: id.clone() },
                class: "text-slate-700 hover:text-slate-900 hover:underline",
                "Open log →"
            }
            span { class: "text-slate-300", "·" }
            span { class: "text-slate-400", "View diff →" }
            span { class: "text-slate-300", "·" }
            span { class: "text-slate-400", "Cancel" }
        }
    }
}

/// Plan section — renders one `StoryCard` per story, or a placeholder.
#[component]
fn PlanSection(plan: Option<crate::domain::Plan>) -> Element {
    rsx! {
        section { class: "space-y-3",
            h2 { class: "text-base font-medium text-slate-800", "Plan" }
            {match plan {
                Some(p) => rsx! {
                    div { class: "space-y-3",
                        for story in p.stories.iter() {
                            StoryCard {
                                key: "{story.id.0}",
                                story: story.clone(),
                                passes: None,
                            }
                        }
                    }
                },
                None => rsx! {
                    div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm",
                        p { class: "text-sm text-slate-500",
                            "Plan not yet generated — task is in Idle or earlier state."
                        }
                    }
                },
            }}
        }
    }
}

/// Review section — findings first, then assertions, or a placeholder.
#[component]
fn ReviewSection(review: Option<crate::domain::Review>) -> Element {
    rsx! {
        section { class: "space-y-3",
            h2 { class: "text-base font-medium text-slate-800", "Review" }
            {match review {
                Some(r) => rsx! {
                    div { class: "space-y-3",
                        for finding in r.findings.iter() {
                            FindingCard {
                                key: "{finding.description}",
                                finding: finding.clone(),
                            }
                        }
                        for (idx, assertion) in r.assertions.iter().enumerate() {
                            AssertionCard {
                                key: "{idx}",
                                assertion: assertion.clone(),
                            }
                        }
                    }
                },
                None => rsx! {
                    div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm",
                        p { class: "text-sm text-slate-500",
                            "Review not yet generated — task hasn't reached the Review phase."
                        }
                    }
                },
            }}
        }
    }
}

/// Receipts section — renders the ReceiptCard, or a placeholder.
#[component]
fn ReceiptsSection(receipts: Option<crate::domain::Receipts>) -> Element {
    rsx! {
        section { class: "space-y-3",
            h2 { class: "text-base font-medium text-slate-800", "Receipts" }
            {match receipts {
                Some(recs) => rsx! { ReceiptCard { receipts: recs } },
                None => rsx! {
                    div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm",
                        p { class: "text-sm text-slate-500",
                            "Task not yet Done — receipts only appear once the Judge phase accepts."
                        }
                    }
                },
            }}
        }
    }
}

/// Loading skeleton — shown while `use_resource` is `Pending`.
#[component]
fn LoadingCard() -> Element {
    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm space-y-2",
            div { class: "h-4 w-24 animate-pulse rounded bg-slate-200" }
            div { class: "h-3 w-full animate-pulse rounded bg-slate-100" }
            div { class: "h-3 w-2/3 animate-pulse rounded bg-slate-100" }
            p { class: "text-xs text-slate-500", "Loading task…" }
        }
    }
}

/// Not-found card — `task_get` returned `Ok(None)` (CLI exited 2).
/// Distinct from the error banner: a not-found is a 404, not a fault.
#[component]
fn NotFoundCard(id: String, workdir: String) -> Element {
    rsx! {
        div { class: "rounded-lg border border-amber-300 bg-amber-50 p-4 shadow-sm space-y-2",
            h3 { class: "text-sm font-medium text-amber-800", "Task not found" }
            p { class: "text-xs text-amber-700",
                "No task with id "
                span { class: "font-mono", "\"{id}\"" }
                " exists in workdir "
                span { class: "font-mono", "{workdir}" }
                "."
            }
            p { class: "text-xs text-amber-700",
                "Run "
                span { class: "font-mono", "alps list --workdir {workdir}" }
                " to see available tasks."
            }
        }
    }
}

/// Error banner — `task_get` returned `Err`. Server-side failure
/// (CLI missing from PATH, JSON drift, etc.).
#[component]
fn ErrorCard(error: String) -> Element {
    rsx! {
        div { class: "rounded-lg border border-rose-300 bg-rose-50 p-4 shadow-sm space-y-2",
            h3 { class: "text-sm font-medium text-rose-800", "Failed to load task" }
            p { class: "text-xs text-rose-700 font-mono break-all", "{error}" }
            p { class: "text-xs text-rose-700",
                "Check that `alps` is on $PATH and that the workdir contains a `tasks/` subdir."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! SSR-mode render tests for TaskDetail's loading/error branches.
    //!
    //! The populated-render branch needs a real `TaskDetail` JSON
    //! round-trip (covered by `verify-us-007.sh` end-to-end). The
    //! loading/error/404 branches are pure render — SSR can exercise
    //! them without touching the network.

    use super::*;
    use dioxus::prelude::*;

    #[test]
    fn loading_card_renders_loading_text() {
        let html = dioxus_ssr::render_element(rsx! {
            LoadingCard {}
        });
        assert!(
            html.contains("Loading task"),
            "LoadingCard should advertise in-flight state: {html}"
        );
    }

    #[test]
    fn not_found_card_renders_id_and_workdir() {
        let html = dioxus_ssr::render_element(rsx! {
            NotFoundCard {
                id: "2026-08-25T120000-deadbeef".to_string(),
                workdir: "/tmp/alps-runs".to_string(),
            }
        });
        assert!(html.contains("Task not found"), "404 card title: {html}");
        assert!(
            html.contains("2026-08-25T120000-deadbeef"),
            "404 card should echo the missing task id: {html}"
        );
        assert!(
            html.contains("/tmp/alps-runs"),
            "404 card should echo the workdir: {html}"
        );
    }

    #[test]
    fn error_card_renders_error_text() {
        let html = dioxus_ssr::render_element(rsx! {
            ErrorCard {
                error: "alps show spawn failed (is `alps` on $PATH?)".to_string(),
            }
        });
        assert!(
            html.contains("Failed to load task"),
            "error banner title: {html}"
        );
        assert!(
            html.contains("alps show spawn failed"),
            "error banner should echo the error: {html}"
        );
    }

    #[test]
    fn format_elapsed_sub_minute() {
        assert_eq!(format_elapsed(0), "0s");
        assert_eq!(format_elapsed(45), "45s");
    }

    #[test]
    fn format_elapsed_sub_hour() {
        assert_eq!(format_elapsed(60), "1m 0s");
        assert_eq!(format_elapsed(3599), "59m 59s");
    }
}