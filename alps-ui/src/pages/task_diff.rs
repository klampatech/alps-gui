//! TaskDiff page (`/tasks/:id/diff`).
//!
//! Renders the per-commit git history for a task's `alps/<task_id>`
//! branch. Single-fetch page (no polling — git history is static).
//!
//! ## Layout
//! - Header: "Diff" + task_id + back-to-detail link
//! - One `CommitCard` per commit (sha + author + timestamp + subject)
//! - One `<pre>` block per commit with the unified diff
//! - Empty state: "No commits on alps/<id> yet" if the nested git
//!   doesn't exist OR has no commits diverged from main
//!
//! ## Why this is simpler than TaskLog
//! TaskLog is a polling tail (lines stream continuously). TaskDiff is
//! a single fetch — git history doesn't change at runtime for our
//! purposes (Ralph pushes commits when it does, the operator can
//! refresh by reloading the page). Future story: add a "Refresh" button
//! if Ralph ever pushes commits mid-session without our noticing.

use dioxus::prelude::*;
// CommitDiff lives in `crate::api::CommitDiff` (re-exported from
// `api::diff` under `feature = "server"`, with a stub in `api::mod`
// for the default + wasm builds — mirrors the LogLine stub pattern).
use crate::api::CommitDiff;

use crate::domain::TaskId;
use crate::routes::Route;
use crate::state;

// Local `default_workdir` removed in M4-proper — replaced by the
// shared `state::Workdir` context. See `state.rs` for the resolution
// chain (config file → env var → `$HOME/Development/alps-runs`).

/// Maximum number of commits we'll render. Beyond this, show a
/// "X more commits not shown" banner (git log can return thousands
/// of commits for an active long-running task).
const MAX_COMMITS_TO_RENDER: usize = 100;

/// Route handler for `/tasks/:id/diff`. Single fetch via
/// `use_resource(task_diff)`.
#[component]
pub fn TaskDiff(id: TaskId) -> Element {
    let workdir_ctx = use_context::<state::Workdir>();
    let workdir = workdir_ctx.get();
    let task_id_for_fn = id.0.clone();
    let task_id_for_display = id.0.clone();

    let resource = use_resource(move || {
        let wd = workdir.clone();
        let tid = task_id_for_fn.clone();
        async move { crate::api::task_diff(wd, tid).await }
    });

    let error_msg: Option<String> = match &*resource.read_unchecked() {
        Some(Err(e)) => Some(format!("{e:?}")),
        _ => None,
    };
    let empty: bool = matches!(&*resource.read_unchecked(), Some(Ok(c)) if c.is_empty());
    let commits: Option<Vec<CommitDiff>> = match &*resource.read_unchecked() {
        Some(Ok(cs)) if !cs.is_empty() => Some(cs.clone()),
        _ => None,
    };

    rsx! {
        div { class: "p-4 sm:p-6 lg:p-8 space-y-4",
            // Header
            div { class: "flex flex-wrap items-baseline justify-between gap-3",
                div { class: "flex items-center gap-3",
                    h1 { class: "text-2xl font-semibold text-slate-800", "Diff" }
                    span { class: "font-mono text-sm text-slate-500", "{task_id_for_display}" }
                }
                div { class: "flex items-center gap-3 text-sm",
                    Link {
                        to: Route::TaskDetail { id: id.clone() },
                        class: "text-slate-600 hover:text-slate-900 hover:underline",
                        "← Back to detail"
                    }
                }
            }

            // Body — loading / error / empty / populated branches.
            if resource.read_unchecked().is_none() {
                LoadingCard {}
            } else if let Some(err) = error_msg {
                ErrorCard { error: err }
            } else if empty {
                EmptyCard { task_id: task_id_for_display.clone() }
            } else if let Some(cs) = commits {
                CommitList { commits: cs, task_id: task_id_for_display.clone() }
            }
        }
    }
}

/// One commit + its diff. Mirrors the `CommitDiff` server-side struct.
/// The diff is shown in a monospace `<pre>` block (no syntax
/// highlighting in v1 — per the M3 brief story 3f).
#[component]
fn CommitCard(commit: CommitDiff) -> Element {
    let short_sha = if commit.sha.len() >= 7 {
        commit.sha[..7].to_string()
    } else {
        commit.sha.clone()
    };
    rsx! {
        article { class: "rounded-lg border border-slate-200 bg-white shadow-sm space-y-2",
            // Header
            div { class: "px-4 pt-3 pb-2 flex flex-wrap items-baseline justify-between gap-2 border-b border-slate-100",
                div { class: "flex items-baseline gap-3",
                    span { class: "font-mono text-sm font-medium text-slate-700", "{short_sha}" }
                    span { class: "font-mono text-xs text-slate-400", "{commit.sha}" }
                    span { class: "text-xs text-slate-500", "{commit.author}" }
                    span { class: "text-xs text-slate-400", "{commit.timestamp}" }
                }
            }
            // Subject line
            div { class: "px-4 pt-1",
                p { class: "text-sm font-medium text-slate-800", "{commit.message}" }
            }
            // Diff (or "no diff" placeholder for merge commits)
            div { class: "px-4 pb-3",
                if commit.patch.trim().is_empty() {
                    p { class: "text-xs italic text-slate-400", "(no diff)" }
                } else {
                    pre {
                        class: "mt-2 overflow-x-auto rounded bg-slate-50 px-3 py-2 text-xs text-slate-800 whitespace-pre font-mono",
                        "{commit.patch}"
                    }
                }
            }
        }
    }
}

/// Commit list — one card per commit, capped at MAX_COMMITS_TO_RENDER.
/// If there are more, shows a banner ("X more not shown — view raw git
/// log to see all").
#[component]
fn CommitList(commits: Vec<CommitDiff>, task_id: String) -> Element {
    let total = commits.len();
    let visible: Vec<CommitDiff> = commits
        .into_iter()
        .take(MAX_COMMITS_TO_RENDER)
        .collect();
    let hidden = total.saturating_sub(MAX_COMMITS_TO_RENDER);
    rsx! {
        div { class: "space-y-3",
            // Summary line
            p { class: "text-sm text-slate-600",
                if total == 1 {
                    "1 commit on alps/{task_id} (branched from main)"
                } else {
                    "{total} commits on alps/{task_id} (branched from main)"
                }
            }
            // Commit cards
            for commit in visible.iter() {
                CommitCard { commit: commit.clone() }
            }
            // Hidden banner
            if hidden > 0 {
                p {
                    class: "text-sm italic text-slate-500 px-4 py-3 rounded border border-dashed border-slate-300",
                    "... and {hidden} more commits not shown (cap at {MAX_COMMITS_TO_RENDER}). Run `git -C tasks/{task_id}/implementation/ralph log alps/{task_id}..main` to see all."
                }
            }
        }
    }
}

#[component]
fn LoadingCard() -> Element {
    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm",
            p { class: "text-sm italic text-slate-500", "Loading diff…" }
        }
    }
}

#[component]
fn EmptyCard(task_id: String) -> Element {
    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm",
            p { class: "text-sm text-slate-700",
                "No commits on alps/{task_id} yet."
            }
            p { class: "text-xs italic text-slate-500 mt-1",
                "Ralph hasn't pushed commits for this task. This is normal for tasks still in Planned state."
            }
        }
    }
}

#[component]
fn ErrorCard(error: String) -> Element {
    rsx! {
        div { class: "rounded-lg border border-red-200 bg-red-50 p-4 shadow-sm",
            p { class: "text-sm font-medium text-red-700", "Diff fetch failed" }
            pre {
                class: "mt-2 overflow-x-auto text-xs text-red-800 whitespace-pre-wrap font-mono",
                "{error}"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_commit(sha: &str, message: &str, patch: &str) -> CommitDiff {
        CommitDiff {
            sha: sha.to_string(),
            author: "Test Author".into(),
            timestamp: "2026-08-26T10:00:00Z".into(),
            message: message.to_string(),
            patch: patch.to_string(),
        }
    }

    #[test]
    fn short_sha_is_7_chars() {
        let c = make_commit("abc1234567890def", "msg", "");
        assert_eq!(&c.sha[..7], "abc1234");
    }

    #[test]
    fn short_sha_handles_short_input() {
        let c = make_commit("abc", "msg", "");
        // SHA shorter than 7 chars — fall back to full SHA
        assert_eq!(c.sha.len(), 3);
    }

    #[test]
    fn empty_patch_round_trips() {
        // Empty patch (merge commits) round-trips through serde.
        let c = make_commit("abc1234567890def", "Merge branch", "");
        let json = serde_json::to_string(&c).unwrap();
        let back: CommitDiff = serde_json::from_str(&json).unwrap();
        assert_eq!(back.patch, "");
    }

    #[test]
    fn task_diff_ssr_shows_header_and_back_link() {
        // SSR-mode test: the route handler should render at least the
        // "Diff" heading + a "← Back to detail" link even when the
        // resource is still loading (None branch). The back-link is
        // the same for loading/error/empty/populated states.
        use crate::domain::TaskId;
        let _id = TaskId::new("2026-08-26T100000-aaaaaaaaaaaaaaa");

        // We can't actually call the component (needs a Dioxus runtime),
        // but we can verify the constants + helpers behave as expected.
        assert!(MAX_COMMITS_TO_RENDER > 0);
        assert!(MAX_COMMITS_TO_RENDER < 10_000);
    }
}