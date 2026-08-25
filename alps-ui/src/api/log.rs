//! Read-side server functions for the per-task log tail panes.
//!
//! M3b: two polled-tail endpoints backing the dual-pane TaskLog page
//! (`/tasks/:id/log`). Both functions read from a file on disk and
//! return the lines that have arrived since the caller's last cursor.
//!
//! ## Endpoints
//!
//! - [`task_log_tail_telemetry`] — reads `<workdir>/.alps-telemetry.log`,
//!   the workdir-wide orchestrator `elog!` stream written by every
//!   running task. Shared across all tasks in the workdir. **Not
//!   filtered by task_id** (the orchestrator's `elog!` macro does not
//!   tag lines; adding tags is a cross-repo `klampatech/alps` change
//!   deferred to v2 — see `~/Obsidian/projects/alps-ui-m3-brief.md`
//!   M3b revision note, 2026-08-25).
//! - [`task_log_tail_ralph`] — reads
//!   `<workdir>/tasks/<id>/implementation/ralph/.ralph-stderr.log`,
//!   the Ralph/Codex subprocess's stderr mirror for this task. Per-task
//!   scoped (one file per task).
//!
//! ## Why polling, not SSE
//!
//! M3b v1 deliberately uses 500ms polling rather than Server-Sent
//! Events. SSE in Dioxus 0.7 requires bypassing the `#[server]` macro
//! (the macro returns a single JSON value via axum, not a streaming
//! response), which adds significant complexity. The polling cadence
//! is fine for a log tail, keeps the verify-script deterministic
//! (no event-loop timing), and lets us upgrade to SSE in v2 without a
//! wire-shape change. See `~/Obsidian/projects/alps-ui-m3-brief.md`
//! M3b "Risks + mitigations" for the full trade-off discussion.
//!
//! ## Why `#[cfg(feature = "server")]` is doubled
//!
//! Same pattern as [`tasks`] and [`run`]: both the module and each
//! function carry the gate so `std::fs` symbols never reach the wasm
//! artifact. Belt-and-suspenders — the macro's own gating is
//! `#[cfg(not(target_arch = "wasm32"))]`, but symbol-level metadata
//! still ends up in the compilation unit unless we exclude the
//! function entirely.
//!
//! ## Why two functions, not one with a source enum
//!
//! Both functions have the same shape (`workdir, since_line_no, -> Vec<LogLine>`)
//! except `task_log_tail_ralph` adds a `task_id` arg. Two separate
//! functions keep the wasm stubs identical-pattern to the existing
//! `tasks_list` / `task_get` / `task_run` trio (one stub per server fn,
//! each a 12-line copy of the helper call). A `source: LogSource` enum
//! would force every caller to branch on the enum in `match` form,
//! which is more code than the savings.
//!
//! ## Cap + cursor semantics
//!
//! Each call returns at most `MAX_LINES_PER_POLL` (500) lines starting
//! at `since_line_no` (0-indexed). The page's polled-tail hook tracks
//! its own cursor per pane (the buffer's `last_seen_line_no + 1` on the
//! next tick). The cap protects against pathological cases (e.g. a
//! burst of 10k Ralph lines arriving between two polls) — the next
//! poll picks up where the cap left off. Memory cap in the hook is
//! 1000 lines per buffer (drop oldest), per the brief.

/// Maximum lines returned per poll. Bounds payload size for bursts.
///
/// 500 was chosen as ~2× the worst-case real-world burst we expect
/// (Ralph's Codex tool emits ~50-200 lines per iteration; orchestrator
/// `elog!` lines are typically <10 per task transition). On the
/// client side, the polled-tail hook caps at 1000 lines in memory
/// per buffer.
pub const MAX_LINES_PER_POLL: usize = 500;

#[cfg(feature = "server")] use std::fs;
#[cfg(feature = "server")] use std::io::{BufRead, BufReader};
#[cfg(feature = "server")] use std::path::PathBuf;

use dioxus_fullstack_core::ServerFnError;
use dioxus_fullstack_macro::server;
use serde::{Deserialize, Serialize};

/// One line from a log file. `line_no` is the 0-indexed file offset
/// (NOT a sequence number — the file's first line is `line_no = 0`).
/// Re-exported from `crate::domain` so pages can name the type
/// without depending on `api::log` directly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogLine {
    pub line_no: u64,
    pub text: String,
}

impl LogLine {
    pub fn new(line_no: u64, text: String) -> Self {
        Self { line_no, text }
    }
}

/// Read `<workdir>/.alps-telemetry.log` starting at `since_line_no` and
/// return up to [`MAX_LINES_PER_POLL`] subsequent lines.
///
/// ## "Missing file" semantics
///
/// Returns `Ok(vec![])` when the file doesn't exist. This is the
/// honest answer for a fresh workdir where no task has ever run —
/// the page should render "no telemetry yet" rather than an error
/// banner. A real read failure (permissions, I/O error on an existing
/// file) DOES surface as `Err`.
///
/// ## Errors
///
/// - The file exists but can't be read (permissions, mid-write race)
///   → `Err(ServerFnError::new(...))` with the OS error.
/// - The cursor points past EOF → `Ok(vec![])`.
#[cfg(feature = "server")]
#[server]
pub async fn task_log_tail_telemetry(
    workdir: String,
    since_line_no: u64,
) -> Result<Vec<LogLine>, ServerFnError> {
    let path = PathBuf::from(&workdir).join(".alps-telemetry.log");
    tail_file(&path, since_line_no, "telemetry").await
}

/// Read `<workdir>/tasks/<task_id>/implementation/ralph/.ralph-stderr.log`
/// starting at `since_line_no` and return up to [`MAX_LINES_PER_POLL`]
/// subsequent lines.
///
/// ## "Missing file" semantics
///
/// Returns `Ok(vec![])` when the file doesn't exist. The Ralph
/// subprocess only writes this file once the task reaches the
/// `[implement]` phase, so a missing file = task hasn't reached
/// Ralph yet = honest "no Ralph activity yet" UX in the page's bottom
/// pane. A real read failure surfaces as `Err`.
///
/// The task_dir exists check is intentionally NOT performed: the
/// file may exist before `<workdir>/tasks/<id>/implementation/`
/// resolves (race during early orchestrator startup), and treating
/// that as "not yet" is consistent with treating missing-file the
/// same way.
///
/// ## Errors
///
/// - The file exists but can't be read → `Err(...)`.
/// - The task_id contains path traversal characters → `Err(...)`
///   before any filesystem access (security: the UI accepts any
///   URL-derived string, so we must reject `../` and similar).
#[cfg(feature = "server")]
#[server]
pub async fn task_log_tail_ralph(
    workdir: String,
    task_id: String,
    since_line_no: u64,
) -> Result<Vec<LogLine>, ServerFnError> {
    // Path-traversal guard. The UI's `Route::TaskLog { id: TaskId }`
    // accepts any URL-derived string (the `FromStr` impl on `TaskId`
    // is infallible), so we MUST validate before joining into a
    // filesystem path. A `../` segment here would let a crafted URL
    // read arbitrary files. Reject `..`, `/`, and null bytes — all
    // invalid in a `YYYY-MM-DDTHHMMSS-<uuid8>` task_id and the
    // only way to escape the tasks/ directory.
    if task_id.contains("..") || task_id.contains('/') || task_id.contains('\\') || task_id.contains('\0') {
        return Err(ServerFnError::new(format!(
            "task_log_tail_ralph: invalid task_id {task_id:?} (must not contain '..', '/', '\\', or null)"
        )));
    }

    let path = PathBuf::from(&workdir)
        .join("tasks")
        .join(&task_id)
        .join("implementation")
        .join("ralph")
        .join(".ralph-stderr.log");
    tail_file(&path, since_line_no, "ralph").await
}

/// Shared read-side body for both fns: skip `since_line_no` lines, then
/// read up to `MAX_LINES_PER_POLL` more.
///
/// Label is a short tag ("telemetry" / "ralph") used in error messages
/// so triage can distinguish which file failed to read.
#[cfg(feature = "server")]
async fn tail_file(
    path: &std::path::Path,
    since_line_no: u64,
    label: &str,
) -> Result<Vec<LogLine>, ServerFnError> {
    // Missing file = Ok(vec![]) per the per-fn "Missing file"
    // semantics documented above.
    if !path.exists() {
        return Ok(Vec::new());
    }

    let file = fs::File::open(path).map_err(|e| {
        ServerFnError::new(format!(
            "task_log_tail_{label}: failed to open {}: {e}",
            path.display()
        ))
    })?;

    let reader = BufReader::new(file);
    let mut out = Vec::with_capacity(MAX_LINES_PER_POLL);

    for (idx, line) in reader.lines().enumerate() {
        let idx = idx as u64;
        if idx < since_line_no {
            // Skip already-seen lines. We can't `skip(N)` on the
            // iterator without buffering, but this is cheap — the
            // OS read-ahead keeps BufReader's cost low even for
            // 1000-line skips.
            continue;
        }
        match line {
            Ok(text) => out.push(LogLine::new(idx, text)),
            Err(e) => {
                return Err(ServerFnError::new(format!(
                    "task_log_tail_{label}: read error at line {idx} of {}: {e}",
                    path.display()
                )));
            }
        }
        if out.len() >= MAX_LINES_PER_POLL {
            break;
        }
    }

    Ok(out)
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;
    use std::io::Write;

    /// Helper: create a temp file with N numbered lines and return its
    /// path. The file is created in a fresh subdirectory of
    /// `std::env::temp_dir()` named after the test so concurrent test
    /// runs don't collide.
    fn write_fixture(name: &str, lines: &[&str]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("alps-ui-log-test-{name}"));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("fixture.log");
        let mut f = std::fs::File::create(&path).expect("create fixture");
        for (i, line) in lines.iter().enumerate() {
            writeln!(f, "line{i}: {line}").expect("write fixture");
        }
        path
    }

    #[tokio::test]
    async fn tail_file_missing_returns_empty() {
        let path = std::env::temp_dir().join("alps-ui-log-test-does-not-exist.log");
        let _ = std::fs::remove_file(&path);
        let result = tail_file(&path, 0, "missing").await.expect("missing file = Ok");
        assert!(result.is_empty(), "missing file must return Ok(vec![]), not Err");
    }

    #[tokio::test]
    async fn tail_file_full_read_from_zero() {
        let path = write_fixture("full", &["alpha", "beta", "gamma"]);
        let result = tail_file(&path, 0, "full").await.expect("read fixture");
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], LogLine::new(0, "line0: alpha".to_string()));
        assert_eq!(result[1], LogLine::new(1, "line1: beta".to_string()));
        assert_eq!(result[2], LogLine::new(2, "line2: gamma".to_string()));
    }

    #[tokio::test]
    async fn tail_file_cursor_skips_seen_lines() {
        let path = write_fixture("cursor", &["a", "b", "c", "d"]);
        let result = tail_file(&path, 2, "cursor").await.expect("read with cursor");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].line_no, 2);
        assert_eq!(result[1].line_no, 3);
    }

    #[tokio::test]
    async fn tail_file_cursor_past_eof_returns_empty() {
        let path = write_fixture("past-eof", &["only"]);
        let result = tail_file(&path, 99, "past-eof").await.expect("cursor past EOF");
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn task_log_tail_ralph_rejects_path_traversal() {
        let result = task_log_tail_ralph(
            "/tmp".to_string(),
            "../../../etc/passwd".to_string(),
            0,
        ).await;
        assert!(result.is_err(), "must reject task_id containing '..'");
        let msg = format!("{:?}", result.unwrap_err());
        assert!(msg.contains("invalid task_id"), "error message must explain the rejection: {msg}");
    }

    #[tokio::test]
    async fn task_log_tail_ralph_rejects_slash() {
        let result = task_log_tail_ralph(
            "/tmp".to_string(),
            "foo/bar".to_string(),
            0,
        ).await;
        assert!(result.is_err(), "must reject task_id containing '/'");
    }
}