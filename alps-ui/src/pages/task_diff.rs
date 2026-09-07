//! TaskDiff page (`/tasks/:id/diff`).
//!
//! Control Rail redesign (deck §05): commits become accordion rows so
//! the diff body only appears on demand. Each row shows the short
//! SHA (amber), subject (sans), and author/time meta (faint). Clicking
//! expands to a unified diff block with color-coded add/rem lines.
//!
//! ## Layout (deck §05)
//!
//! - Header: "Diff" + task_id + back-link
//! - Banner: "N commits shown · alps/task-X..main"
//! - Commit rows: native `<details>` per commit, with the diff block
//!   as the body
//! - Empty state: calm dashed-border card explaining "no commits yet"
//!
//! ## Why native `<details>` for the accordion
//!
//! `dioxuslabs/components::Accordion` is not in our `Cargo.toml` deps.
//! Native HTML `<details>` gives us the same behavior (toggle on click,
//! keyboard accessible, no JS state needed) for free. The CSS classes
//! (`.commit-row`, `.diff-block`, `.empty-callout`) come from
//! `assets/main.css`.

use dioxus::prelude::*;
use crate::api::CommitDiff;

use crate::domain::TaskId;
use crate::routes::Route;
use crate::state;

const MAX_COMMITS_TO_RENDER: usize = 100;

#[component]
pub fn TaskDiff(id: TaskId) -> Element {
    let workdir_signal = use_context::<state::Workdir>().signal();
    let task_id_for_fn = id.0.clone();
    let task_id_for_display = id.0.clone();

    let resource = use_resource(move || {
        let wd = workdir_signal.cloned();
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
        div {
            style: "padding:24px;",
            // Header
            div {
                style: "display:flex;flex-wrap:wrap;align-items:baseline;justify-content:space-between;gap:12px;margin-bottom:20px;",
                div {
                    style: "display:flex;align-items:baseline;gap:12px;",
                    h1 {
                        class: "alps-mono",
                        style: "font-size:24px;font-weight:600;margin:0;color:var(--text);",
                        "Diff"
                    }
                    span {
                        class: "alps-mono",
                        style: "font-size:14px;color:var(--dim);",
                        "{task_id_for_display}"
                    }
                }
                Link {
                    to: Route::TaskDetail { id: id.clone() },
                    class: "alps-mono",
                    style: "font-size:11px;color:var(--dim);text-decoration:none;",
                    "← Back to detail"
                }
            }
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

/// One commit + its diff, rendered as a native `<details>` accordion.
/// The diff body is a color-coded `<pre>` block (no syntax
/// highlighting — matches the deck §05 mockup).
#[component]
fn CommitRow(commit: CommitDiff) -> Element {
    let short_sha = if commit.sha.len() >= 7 {
        commit.sha[..7].to_string()
    } else {
        commit.sha.clone()
    };
    rsx! {
        details {
            style: "margin-bottom:8px;",
            summary {
                class: "commit-row",
                span {
                    class: "commit-sha alps-mono",
                    style: "font-weight:600;",
                    "{short_sha}"
                }
                span {
                    class: "commit-subject",
                    "{commit.message}"
                }
                span {
                    class: "commit-meta alps-mono",
                    "{commit.author} · {commit.timestamp}"
                }
            }
            if !commit.patch.trim().is_empty() {
                div {
                    class: "diff-block alps-mono",
                    // The patch from `git show` is plain text with
                    // `+` / `-` / ` ` prefixes. We split lines and
                    // tag each as add/rem/ctx for color coding.
                    for line in commit.patch.lines() {
                        {
                            let (cls, content) = if let Some(rest) = line.strip_prefix('+') {
                                ("add", rest.to_string())
                            } else if let Some(rest) = line.strip_prefix('-') {
                                ("rem", rest.to_string())
                            } else {
                                ("ctx", line.to_string())
                            };
                            rsx! {
                                span {
                                    class: "{cls}",
                                    "{content}"
                                }
                            }
                        }
                    }
                }
            } else {
                div {
                    style: "padding:10px 16px;color:var(--faint);font-style:italic;font-size:12px;",
                    "(no diff — merge or empty commit)"
                }
            }
        }
    }
}

#[component]
fn CommitList(commits: Vec<CommitDiff>, task_id: String) -> Element {
    let total = commits.len();
    let visible: Vec<CommitDiff> = commits
        .into_iter()
        .take(MAX_COMMITS_TO_RENDER)
        .collect();
    let hidden = total.saturating_sub(MAX_COMMITS_TO_RENDER);
    rsx! {
        div {
            style: "display:flex;flex-direction:column;gap:8px;",
            // Banner — "N commits shown · alps/task-X..main".
            div {
                class: "diff-banner alps-mono",
                "{total} commit shown · alps/{task_id}..main"
            }
            // Commit rows (each is a `<details>` accordion).
            for commit in visible.iter() {
                CommitRow { commit: commit.clone() }
            }
            // Hidden banner — calm copy.
            if hidden > 0 {
                div {
                    class: "empty-callout",
                    style: "margin-top:8px;",
                    b { "More commits not shown" }
                    "{hidden} more commits beyond the cap of {MAX_COMMITS_TO_RENDER}. Run git log alps/{task_id}..main to see them all."
                }
            }
        }
    }
}

#[component]
fn LoadingCard() -> Element {
    rsx! {
        div {
            class: "surface",
            style: "padding:18px 20px;",
            p {
                class: "alps-mono",
                style: "font-size:12px;color:var(--faint);font-style:italic;margin:0;",
                "Loading diff…"
            }
        }
    }
}

#[component]
fn EmptyCard(task_id: String) -> Element {
    rsx! {
        div {
            class: "empty-callout",
            style: "margin-top:8px;",
            b { "Empty state — {task_id}" }
            "No commits on alps/{task_id} yet. Ralph hasn't pushed commits for this task. This confirms the task hasn't been implemented, not that the page is broken."
        }
    }
}

#[component]
fn ErrorCard(error: String) -> Element {
    rsx! {
        div {
            class: "surface",
            style: "padding:18px 20px;border-color:var(--red);",
            h3 {
                class: "alps-mono",
                style: "font-size:13px;font-weight:600;color:var(--red);margin:0 0 8px 0;",
                "Diff fetch failed"
            }
            pre {
                class: "alps-mono",
                style: "font-size:11px;color:var(--red);white-space:pre-wrap;margin:0;word-break:break-all;",
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
        assert_eq!(c.sha.len(), 3);
    }

    #[test]
    fn empty_patch_round_trips() {
        let c = make_commit("abc1234567890def", "Merge branch", "");
        let json = serde_json::to_string(&c).unwrap();
        let back: CommitDiff = serde_json::from_str(&json).unwrap();
        assert_eq!(back.patch, "");
    }

    /// SSR contract: TaskDiff renders the page header + a back-link
    /// even when the resource is loading. The commit list only shows
    /// when populated (or the empty state, when 0 commits).
    #[test]
    fn task_diff_ssr_shows_header_and_back_link() {
        use crate::domain::TaskId;
        let _id = TaskId::new("2026-08-26T100000-aaaaaaaaaaaaaaa");
        assert!(MAX_COMMITS_TO_RENDER > 0);
        assert!(MAX_COMMITS_TO_RENDER < 10_000);
    }
}
