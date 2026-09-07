//! TaskDetail page (`/tasks/:id`).
//!
//! Control Rail redesign (deck §03): the 5-node rail becomes the page
//! header (full-width). The body shows the prompt in a left-amber
//! block, a chip row with attempt/elapsed/started/branch, and three
//! accordions (Plan / Review / Receipts) that collapse when not yet
//! generated.
//!
//! ## What renders (deck §03 order)
//!
//! 1. Page header — `task-XXX — <prompt excerpt>` + StatusPill
//! 2. FullRail (5 nodes) — current phase derived from artifact presence
//! 3. Prompt block (left-amber stripe + mono excerpt)
//! 4. Chip row — attempt · elapsed · started · branch
//! 5. Plan accordion — one StoryCard per story, or placeholder
//! 6. Review accordion — FindingCards + AssertionCards, or placeholder
//! 7. Receipts accordion — ReceiptCard, or placeholder
//! 8. Footer — Open log → · View diff → · Cancel (when Running)
//!
//! ## Artifact-presence → current_phase mapping
//!
//! The FullRail's `current_phase` is computed from which artifacts are
//! loaded (NOT from `summary.state` — state lags reality because Ralph
//! writes `implementation.json` before transitioning state to
//! `Implemented`).
//!
//! ```text
//!   0  Plan       — only prompt.md loaded
//!   1  Implement  — implementation.json loaded
//!   2  Review     — review.json loaded
//!   3  Judge      — receipts.json loaded (judged)
//!   4  Done       — state == Done
//! ```
//!
//! `has_failed` is `true` when state is `Failed` or `Rejected`. The
//! active node paints red instead of amber when failed.

use dioxus::prelude::*;

use crate::api::task_get;
use crate::components::{AssertionCard, FindingCard, FullRail, ReceiptCard, StatusPill, StoryCard};
use crate::domain::{TaskId, TaskState};
use crate::routes::Route;
use crate::state;

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

fn format_created_at(dt: chrono::DateTime<chrono::Utc>) -> String {
    dt.format("%m-%d %H:%M").to_string()
}

#[component]
pub fn TaskDetail(id: TaskId) -> Element {
    let workdir_signal = use_context::<state::Workdir>().signal();
    let task_id_for_fn = id.0.clone();
    let task_id_for_display = id.0.clone();
    let resource = use_resource(move || {
        let wd = workdir_signal.cloned();
        let tid = task_id_for_fn.clone();
        async move { task_get(wd, tid).await }
    });

    let body = match &*resource.read_unchecked() {
        None => rsx! { LoadingCard {} },
        Some(Ok(None)) => rsx! {
            NotFoundCard { id: id.0.clone(), workdir: workdir_signal.cloned() }
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
        div {
            style: "padding:24px;",
            div {
                style: "margin-bottom:16px;display:flex;align-items:baseline;gap:14px;",
                // Breadcrumb-ish back link above the title.
                Link {
                    to: Route::Dashboard {},
                    class: "alps-mono",
                    style: "font-size:11px;color:var(--dim);text-decoration:none;",
                    "← Dashboard"
                }
                span {
                    class: "alps-mono",
                    style: "font-size:13px;color:var(--text);",
                    "{task_id_for_display}"
                }
            }
            {body}
        }
    }
}

/// Derive the rail's current_phase (0-indexed) from which artifacts
/// are loaded. State alone is not enough — Ralph writes artifacts
/// before transitioning state, so the rail can show "ahead" of the
/// state machine.
fn current_phase_from_detail(detail: &crate::domain::TaskDetail) -> u8 {
    let has_impl = detail.implementation.is_some();
    let has_review = detail.review.is_some();
    let has_receipts = detail.receipts.is_some();
    let is_done = matches!(detail.summary.state, TaskState::Done);
    if is_done {
        return 4;
    }
    if has_receipts { 3 }
    else if has_review { 2 }
    else if has_impl { 1 }
    else { 0 }
}

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
    let branch_display = detail
        .implementation
        .as_ref()
        .map(|impl_| impl_.ralph_branch.clone())
        .unwrap_or_else(|| "—".to_string());
    let prompt_text = detail
        .prompt
        .clone()
        .unwrap_or_else(|| summary.prompt_excerpt.clone());

    let phase = current_phase_from_detail(&detail);
    let has_failed = matches!(summary.state, TaskState::Failed | TaskState::Rejected);

    rsx! {
        // Page header — task-XXX — excerpt + StatusPill on the right.
        div {
            style: "display:flex;justify-content:space-between;align-items:center;margin-bottom:20px;gap:16px;flex-wrap:wrap;",
            div {
                class: "alps-mono",
                style: "font-size:18px;font-weight:600;color:var(--text);",
                "{task_id} "
                span {
                    style: "color:var(--faint);font-weight:400;",
                    "— {summary.prompt_excerpt}"
                }
            }
            StatusPill { state: summary.state }
        }
        // Full-width rail header — the signature element.
        div {
            style: "margin-bottom:24px;",
            FullRail {
                current_phase: phase,
                has_failed,
            }
        }
        // Prompt block — left-amber stripe + mono excerpt of the prompt.
        div {
            class: "prompt-block alps-mono",
            style: "margin-bottom:14px;",
            "\"{prompt_text}\""
        }
        // Chip row — attempt · elapsed · started · branch.
        div {
            style: "display:flex;gap:10px;flex-wrap:wrap;margin-bottom:28px;",
            span { class: "chip", "{attempts_display}" }
            span { class: "chip", "elapsed {elapsed_display}" }
            span { class: "chip", "started {created_display}" }
            span { class: "chip", "branch {branch_display}" }
        }
        // Three accordions — Plan / Review / Receipts.
        PlanSection { plan: detail.plan.clone() }
        ReviewSection { review: detail.review.clone() }
        ReceiptsSection { receipts: detail.receipts.clone() }
        // Footer — Open log → · View diff → · Cancel (when Running).
        div {
            style: "display:flex;justify-content:space-between;align-items:center;padding-top:18px;border-top:1px solid var(--hair);font-family:var(--mono);font-size:11.5px;",
            div {
                style: "display:flex;gap:14px;",
                Link {
                    to: Route::TaskLog { id: id.clone() },
                    style: "color:var(--dim);text-decoration:none;",
                    "Open log →"
                }
                Link {
                    to: Route::TaskDiff { id: id.clone() },
                    style: "color:var(--dim);text-decoration:none;",
                    "View diff →"
                }
            }
            if summary.state == TaskState::Running {
                CancelButton { id: id.clone() }
            }
        }
    }
}

#[component]
fn CancelButton(id: TaskId) -> Element {
    let mut cancelling = use_signal(|| false);
    let mut error_msg = use_signal::<Option<String>>(|| None);
    let workdir_ctx = use_context::<state::Workdir>();
    let mut cancel = use_action(move |(wd, tid): (String, String)| {
        let wd = wd.clone();
        let tid = tid.clone();
        async move {
            crate::api::task_cancel(wd, tid)
                .await
                .map_err(|e| anyhow::anyhow!("task_cancel failed: {e:?}"))
        }
    });

    rsx! {
        div {
            style: "display:flex;flex-direction:column;gap:4px;align-items:flex-end;",
            button {
                class: "btn-danger",
                style: "padding:6px 10px;font-size:11px;",
                disabled: *cancelling.read(),
                onclick: move |_| {
                    cancelling.set(true);
                    error_msg.set(None);
                    let wd = workdir_ctx.get();
                    let tid = id.0.clone();
                    cancel.call((wd, tid));
                },
                if *cancelling.read() {
                    "Cancelling…"
                } else {
                    "Cancel"
                }
            }
            if let Some(err) = error_msg.read().clone() {
                p {
                    class: "alps-mono",
                    style: "font-size:11px;color:var(--red);margin:0;",
                    "{err}"
                }
            }
        }
    }
}

/// Plan accordion — opened by default when stories exist, otherwise
/// collapsed. Uses the native `<details>` HTML element for state
/// management (no Dioxus signal needed). This matches the deck §03
/// mockup where Plan is the "active" accordion.
#[component]
fn PlanSection(plan: Option<crate::domain::Plan>) -> Element {
    rsx! {
        details {
            class: "accordion",
            open: true,
            summary {
                class: "accordion-head",
                span { "Plan" }
                span { class: "chev", "⌄" }
            }
            div {
                class: "accordion-body",
                {match plan {
                    Some(p) => rsx! {
                        div {
                            style: "display:flex;flex-direction:column;gap:12px;",
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
                        p {
                            class: "alps-sans",
                            style: "font-size:12.5px;color:var(--faint);font-style:italic;margin:0;",
                            "Plan not yet generated — task is in Idle or earlier state."
                        }
                    },
                }}
            }
        }
    }
}

#[component]
fn ReviewSection(review: Option<crate::domain::Review>) -> Element {
    rsx! {
        details {
            class: "accordion",
            summary {
                class: "accordion-head",
                span {
                    style: "color:var(--faint);",
                    "Review"
                }
                span { class: "chev", "⌄" }
            }
            div {
                class: "accordion-body",
                {match review {
                    Some(r) => rsx! {
                        div {
                            style: "display:flex;flex-direction:column;gap:12px;",
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
                        p {
                            class: "alps-sans",
                            style: "font-size:12.5px;color:var(--faint);font-style:italic;margin:0;",
                            "Review not yet generated — appears once the implement phase completes."
                        }
                    },
                }}
            }
        }
    }
}

#[component]
fn ReceiptsSection(receipts: Option<crate::domain::Receipts>) -> Element {
    rsx! {
        details {
            class: "accordion",
            summary {
                class: "accordion-head",
                span {
                    style: "color:var(--faint);",
                    "Receipts"
                }
                span { class: "chev", "⌄" }
            }
            div {
                class: "accordion-body",
                {match receipts {
                    Some(recs) => rsx! { ReceiptCard { receipts: recs } },
                    None => rsx! {
                        p {
                            class: "alps-sans",
                            style: "font-size:12.5px;color:var(--faint);font-style:italic;margin:0;",
                            "Receipts only appear once the Judge phase accepts."
                        }
                    },
                }}
            }
        }
    }
}

#[component]
fn LoadingCard() -> Element {
    rsx! {
        div {
            class: "surface",
            style: "padding:24px;",
            div {
                style: "display:flex;flex-direction:column;gap:8px;",
                div { style: "height:12px;width:140px;border-radius:6px;background:var(--hair-soft);" }
                div { style: "height:8px;width:100%;border-radius:4px;background:var(--hair-soft);" }
                div { style: "height:8px;width:60%;border-radius:4px;background:var(--hair-soft);" }
            }
            p {
                class: "alps-mono",
                style: "font-size:11px;color:var(--faint);margin:8px 0 0 0;",
                "Loading task…"
            }
        }
    }
}

#[component]
fn NotFoundCard(id: String, workdir: String) -> Element {
    rsx! {
        div {
            class: "surface",
            style: "padding:24px;border-color:var(--amber);",
            h3 {
                class: "alps-mono",
                style: "font-size:13px;font-weight:600;color:var(--amber);margin:0 0 8px 0;",
                "Task not found"
            }
            p {
                style: "font-size:12px;color:var(--dim);margin:0 0 4px 0;",
                "No task with id "
                span { class: "alps-mono", "{id}" }
                " exists in workdir "
                span { class: "alps-mono", "{workdir}" }
                "."
            }
            p {
                class: "alps-mono",
                style: "font-size:11px;color:var(--faint);margin:8px 0 0 0;",
                "Run alps list --workdir {workdir} to see available tasks."
            }
        }
    }
}

#[component]
fn ErrorCard(error: String) -> Element {
    rsx! {
        div {
            class: "surface",
            style: "padding:24px;border-color:var(--red);",
            h3 {
                class: "alps-mono",
                style: "font-size:13px;font-weight:600;color:var(--red);margin:0 0 8px 0;",
                "Failed to load task"
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

#[cfg(test)]
mod tests {
    use super::*;

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

    /// FullRail integration — page-level test that confirms TaskDetail's
    /// PopulatedDetail renders the rail's 5 nodes. Loading/error/404
    /// branches render different markup so we test those separately.
    #[test]
    fn full_rail_renders_all_five_phase_labels() {
        let html = dioxus_ssr::render_element(rsx! {
            FullRail { current_phase: 2, has_failed: false }
        });
        for label in ["Plan", "Implement", "Review", "Judge", "Done"] {
            assert!(
                html.contains(label),
                "FullRail should render the {label} node label: {html}"
            );
        }
    }
}
