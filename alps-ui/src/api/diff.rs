//! `task_diff` server fn (M3c, story 3c.1).
//!
//! Returns the per-commit git history for a task's `alps/<task_id>`
//! branch, including the unified diff for each commit.
//!
//! ## Where the git repo actually lives
//!
//! **Preflight finding (2026-08-26):** the M3 brief assumed
//! `git -C <workdir> log ...` — i.e. top-level git on the workdir.
//! But `~/Development/alps-runs/` (the canonical workdir) is NOT a
//! git repo. Each task has its own nested git at
//! `<workdir>/tasks/<id>/implementation/ralph/.git` (created by
//! `alps-core/src/git_ops.rs:run_git` when Ralph starts implementing).
//! The branch name is `alps/<task_id>` (canonical, per the
//! `.last-branch` file in the task dir) — same as main if no Ralph
//! commits have happened, diverged once Ralph starts.
//!
//! So `task_diff` does `git -C <workdir>/tasks/<id>/implementation/ralph ...`
//! (verified the path exists for every task with Ralph history;
//! returns empty `Vec<CommitDiff>` for tasks whose Ralph hasn't
//! initialized git yet).
//!
//! ## What `task_diff` returns
//!
//! A `Vec<CommitDiff>` where each entry has:
//! - `sha`: full 40-char commit hash
//! - `author`: author name
//! - `timestamp`: ISO 8601 UTC
//! - `message`: commit subject (first line)
//! - `patch`: unified diff (empty for merge commits / no-diff commits)
//!
//! Returns `Vec::new()` if the nested git dir doesn't exist OR the
//! branch doesn't exist (e.g. task is still in `Planned` state and
//! Ralph never started). Returns `Err` only on actual `git` command
//! failures (network, permissions, etc.).
//!
//! ## Why shell out to `git` instead of using the `git2` crate
//!
//! M2's `task_run` shells out to `alps` via `tokio::process::Command`
//! (same precedent). Adding `git2` as a dependency for one server fn
//! is overkill — the workdir-bound `git` binary is always on $PATH,
//! and the output format we need (custom log format + show) is
//! straightforward to parse.
//!
//! ## Security: path-traversal guard
//!
//! `task_id` arrives as a URL-derived string (typed `TaskId` from
//! `Route::TaskDiff`, but the parser is infallible). Defensively
//! reject before any FS access — same pattern as `task_log_tail_ralph`
//! (Pitfall from M3b).

use std::path::Path;
use std::process::Stdio;

use dioxus_fullstack_core::ServerFnError;
use dioxus_fullstack_macro::server;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};

/// One commit + its unified diff.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommitDiff {
    /// Full 40-char commit SHA.
    pub sha: String,
    /// Author name (from git's `author` field).
    pub author: String,
    /// ISO 8601 UTC commit timestamp.
    pub timestamp: String,
    /// Subject line of the commit message (first line, no body).
    pub message: String,
    /// Unified diff (`git show <sha>` output, no-commit-header form).
    /// Empty for merge commits or pure-merge changes.
    pub patch: String,
}

/// Return the per-commit git history for a task's `alps/<task_id>`
/// branch.
///
/// Lookups the nested git at
/// `<workdir>/tasks/<task_id>/implementation/ralph/.git` and runs:
/// 1. `git -C <ralph_dir> log --format='%H%n%an%n%aI%n%s' alps/<id>..main`
/// 2. For each parsed commit, `git -C <ralph_dir> show <sha> --no-color --pretty=format:`
///
/// Returns `Ok(Vec::new())` when the nested git dir doesn't exist or
/// the branch hasn't diverged from main (no commits yet).
#[cfg(feature = "server")]
#[server]
pub async fn task_diff(
    workdir: String,
    task_id: String,
) -> Result<Vec<CommitDiff>, ServerFnError> {
    // Path-traversal guard.
    if task_id.contains("..") || task_id.contains('/')
        || task_id.contains('\\') || task_id.contains('\0')
    {
        return Err(ServerFnError::new(format!(
            "task_diff: invalid task_id {task_id:?}"
        )));
    }

    let workdir_path = Path::new(&workdir);
    let ralph_dir = workdir_path
        .join("tasks")
        .join(&task_id)
        .join("implementation")
        .join("ralph");
    let git_dir = ralph_dir.join(".git");

    if !git_dir.exists() {
        // No nested git yet (Ralph hasn't started, or workdir is
        // fresh). Treat as empty diff.
        return Ok(Vec::new());
    }

    let branch = format!("alps/{task_id}");

    // Step 1: list commits. Format: each commit occupies 4 lines
    // (sha, author, timestamp, subject), separated by a blank line.
    // We pick `\u{1e}` (ASCII Record Separator) as the separator to
    // avoid collisions with message bodies that could contain
    // anything. Then split on it.
    let log_output = tokio::process::Command::new("git")
        .arg("-C")
        .arg(&ralph_dir)
        .arg("log")
        .arg("--no-merges")
        .arg(format!("--format=%H%n%an%n%aI%n%s%n%x1e"))
        .arg(format!("{branch}..main"))
        .output()
        .await
        .map_err(|e| {
            ServerFnError::new(format!("task_diff: git log failed: {e}"))
        })?;

    if !log_output.status.success() {
        let stderr = String::from_utf8_lossy(&log_output.stderr).to_string();
        // `git log X..Y` returns 128 with "fatal: ambiguous argument"
        // when X or Y doesn't resolve. Treat as empty diff.
        if stderr.contains("unknown revision")
            || stderr.contains("ambiguous argument")
            || stderr.contains("does not have any commits yet")
            || stderr.contains("bad revision")
        {
            return Ok(Vec::new());
        }
        return Err(ServerFnError::new(format!(
            "task_diff: git log {branch}..main failed: {stderr}"
        )));
    }

    let log_text = String::from_utf8_lossy(&log_output.stdout);
    let mut commits = Vec::new();
    for record in log_text.split('\u{1e}').filter(|r| !r.trim().is_empty()) {
        let lines: Vec<&str> = record.lines().collect();
        if lines.len() < 4 {
            // Malformed record — skip rather than fail the whole call.
            continue;
        }
        commits.push(CommitDiff {
            sha: lines[0].to_string(),
            author: lines[1].to_string(),
            timestamp: lines[2].to_string(),
            message: lines[3].to_string(),
            patch: String::new(), // filled in step 2
        });
    }

    // Step 2: per-commit diff. `git show <sha>` returns the diff +
    // a header; we suppress the header with `--pretty=format:`.
    for commit in &mut commits {
        let show_output = tokio::process::Command::new("git")
            .arg("-C")
            .arg(&ralph_dir)
            .arg("show")
            .arg(&commit.sha)
            .arg("--no-color")
            .arg("--pretty=format:")
            .output()
            .await
            .map_err(|e| {
                ServerFnError::new(format!(
                    "task_diff: git show {} failed: {e}",
                    commit.sha
                ))
            })?;

        if show_output.status.success() {
            commit.patch = String::from_utf8_lossy(&show_output.stdout).to_string();
        }
        // If `git show` fails (e.g. a merge commit), leave patch empty.
    }

    Ok(commits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_diff_serializes_round_trip() {
        let c = CommitDiff {
            sha: "abc123".into(),
            author: "Kyle".into(),
            timestamp: "2026-08-26T10:00:00Z".into(),
            message: "fix: foo".into(),
            patch: "diff --git a/foo b/foo\n".into(),
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: CommitDiff = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn empty_patch_is_preserved() {
        // Merge commits + no-diff commits produce empty patches.
        let c = CommitDiff {
            sha: "dead".into(),
            author: "ALPS".into(),
            timestamp: "2026-08-26T10:00:00Z".into(),
            message: "alps: initial setup".into(),
            patch: String::new(),
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: CommitDiff = serde_json::from_str(&json).unwrap();
        assert_eq!(back.patch, "");
    }
}