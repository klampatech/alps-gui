//! Cancel-related server fns: `task_cancel` (story 3c.3) + supporting
//! file-format parsing for `.alps-pids.json`.
//!
//! ## Why this module exists
//!
//! M3c lands the Cancel button on TaskDetail (story 3c.4). The cancel
//! path needs to find the OS PID of the `alps run` subprocess that
//! `task_run` spawned for a given task_id, then send SIGTERM to it.
//!
//! ## Lookup strategy (Kyle-approved Option C, 2026-08-26)
//!
//! 1. **First**: check the in-memory process registry
//!    (`process_registry::take`) — covers the common case where
//!    `task_cancel` and the original `task_run` are running in the
//!    same alps-ui server process.
//! 2. **Fallback**: read `<workdir>/.alps-pids.json` from disk —
//!    covers the cross-restart case where the server restarted and
//!    the in-memory map is gone but the file is still there.
//! 3. If both miss: return `Err` ("no such task"). If we find the
//!    entry but `kill -TERM` returns ESRCH (process already reaped),
//!    surface "task already completed" as a separate error code so
//!    the UI can show a friendlier message.
//!
//! ## The child process is detached
//!
//! `task_run` does NOT call `child.wait()` — the orchestrator runs
//! for minutes-to-hours. When the orchestrator exits naturally
//! (Done / Failed / Rejected), the `Child` handle's `try_wait` will
//! report `Some(ExitStatus)`. M3c doesn't try to detect natural exit
//! explicitly — if the user clicks Cancel after the orchestrator has
//! finished, the `kill -TERM` will fail with ESRCH and we'll surface
//! "task already completed".

use std::path::Path;

use dioxus_fullstack_core::ServerFnError;
use dioxus_fullstack_macro::server;
use serde::Deserialize;

/// Cancel the orchestrator process for `task_id`.
///
/// Looks up the PID via the in-memory registry first, falls back to
/// the on-disk `.alps-pids.json` file. Sends SIGTERM via the standard
/// `kill` shell command (avoids depending on the `nix` crate; the
/// workdir-bound `kill` binary is always on $PATH).
///
/// Returns `Ok(())` on successful signal delivery. Returns `Err` if:
/// - No matching entry in registry or file (server restarted AND file
///   pruned/lost the entry)
/// - `kill -TERM` returned ESRCH (process already reaped / never
///   started)
/// - File I/O error reading `.alps-pids.json`
#[cfg(feature = "server")]
#[server]
pub async fn task_cancel(
    workdir: String,
    task_id: String,
) -> Result<(), ServerFnError> {
    // Path-traversal guard: task_id arrives as a URL-derived string
    // (Route::TaskDiff uses it typed; the cancel button passes the
    // typed TaskId through). Defensively reject before any FS
    // access.
    if task_id.contains("..") || task_id.contains('/')
        || task_id.contains('\\') || task_id.contains('\0')
    {
        return Err(ServerFnError::new(format!(
            "task_cancel: invalid task_id {task_id:?}"
        )));
    }

    let workdir_path = std::path::PathBuf::from(&workdir);

    // 1. In-memory registry: take the Child + metadata.
    let child_info = crate::api::process_registry::take(&task_id);

    // 2. Fallback: on-disk file. Re-read every time because another
    // alps-ui server may have written it since startup.
    let file_info = read_alps_pids_entry(&workdir_path, &task_id)?;

    let (pid, started_at) = match (child_info, file_info) {
        // Memory hit — use it (most common case).
        (Some((_child, pid, started_at, _workdir)), _) => {
            // Note: we already removed the entry from the registry
            // via `take()`; don't re-write the file here until the
            // SIGTERM succeeds (see below).
            (pid, Some(started_at))
        }
        // File-only hit — server restart scenario.
        (None, Some((pid, started_at))) => (pid, started_at),
        // Neither.
        (None, None) => {
            return Err(ServerFnError::new(format!(
                "task_cancel: no such task {task_id:?}"
            )));
        }
    };

    // Send SIGTERM via the shell `kill` command. We don't import the
    // `nix` crate or use `libc::kill` directly because the workdir-
    // bound `kill` binary is always on $PATH and gives us a clean
    // error path (non-zero exit on ESRCH).
    let output = tokio::process::Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .output()
        .await
        .map_err(|e| {
            ServerFnError::new(format!(
                "task_cancel: failed to spawn kill(1): {e}"
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        // ESRCH = "No such process" — the orchestrator exited
        // naturally between the registry lookup and our signal.
        if stderr.contains("No such process") || output.status.code() == Some(1) {
            // Clean up the on-disk file so the user gets a fresh
            // error next time, not the same ghost entry.
            let _ = prune_alps_pids_entry(&workdir_path, &task_id);
            return Err(ServerFnError::new(format!(
                "task_cancel: task {task_id:?} already completed (ESRCH)"
            )));
        }
        return Err(ServerFnError::new(format!(
            "task_cancel: kill -TERM {pid} failed: {stderr}"
        )));
    }

    // SIGTERM delivered successfully. Clean up both registries.
    // (The in-memory entry was already removed by `take()` above;
    // here we only need to prune the on-disk file.)
    if let Err(e) = prune_alps_pids_entry(&workdir_path, &task_id) {
        eprintln!(
            "task_cancel: warning — failed to prune {}/.alps-pids.json: {e}",
            workdir_path.display()
        );
    }

    let started_str = started_at
        .map(|s| s.to_rfc3339())
        .unwrap_or_else(|| "<unknown>".to_string());
    eprintln!(
        "task_cancel: sent SIGTERM to pid={pid} (started_at={started_str}) for task_id={task_id:?}"
    );

    Ok(())
}

/// On-disk representation of `<workdir>/.alps-pids.json`.
///
/// Mirrors the writer in `task_run` (same module). If the shapes ever
/// drift, this struct is the ground truth for what `task_cancel` can
/// consume.
#[derive(Debug, Deserialize)]
struct PidFile {
    tasks: Vec<PidEntry>,
}

#[derive(Debug, Deserialize)]
struct PidEntry {
    task_id: String,
    pid: u32,
    #[allow(dead_code)] // Future story may surface this in the receipt card
    started_at: String,
}

/// Read `<workdir>/.alps-pids.json` and return the entry for
/// `task_id` (if any). Returns `Ok(None)` if the file is missing OR
/// exists but doesn't contain this task_id.
///
/// File-missing is `Ok(None)` (not `Err`) because the file is
/// optional infrastructure — a fresh workdir has no orchestrator
/// processes to track.
fn read_alps_pids_entry(
    workdir: &Path,
    task_id: &str,
) -> Result<Option<(u32, Option<chrono::DateTime<chrono::Utc>>)>, ServerFnError> {
    let path = workdir.join(".alps-pids.json");
    let body = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(ServerFnError::new(format!(
                "task_cancel: read {}: {e}",
                path.display()
            )));
        }
    };

    let parsed: PidFile = serde_json::from_str(&body).map_err(|e| {
        ServerFnError::new(format!(
            "task_cancel: parse {}: {e}",
            path.display()
        ))
    })?;

    for entry in parsed.tasks {
        if entry.task_id == task_id {
            let started_at = chrono::DateTime::parse_from_rfc3339(&entry.started_at)
                .ok()
                .map(|d| d.with_timezone(&chrono::Utc));
            return Ok(Some((entry.pid, started_at)));
        }
    }
    Ok(None)
}

/// Remove the entry for `task_id` from `<workdir>/.alps-pids.json`
/// (atomic write). Called after a successful SIGTERM so the file
/// doesn't carry ghost entries.
///
/// If the file doesn't exist OR has no entries for this task_id,
/// returns `Ok(())` (idempotent).
fn prune_alps_pids_entry(workdir: &Path, task_id: &str) -> std::io::Result<()> {
    let path = workdir.join(".alps-pids.json");
    let body = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };

    let parsed: PidFile = match serde_json::from_str::<PidFile>(&body) {
        Ok(p) => p,
        // If the file is corrupt, leave it alone — the user can
        // inspect / rm it manually. Better to surface no false
        // "already pruned" than to silently lose entries.
        Err(_) => return Ok(()),
    };

    let kept: Vec<&PidEntry> = parsed
        .tasks
        .iter()
        .filter(|e| e.task_id != task_id)
        .collect();

    if kept.len() == parsed.tasks.len() {
        // Nothing to prune.
        return Ok(());
    }

    // Re-serialize and atomic-rename. Same temp-file pattern as
    // task_run's writer.
    #[derive(serde::Serialize)]
    struct PidFileOut<'a> {
        tasks: Vec<PidEntryOut<'a>>,
    }
    #[derive(serde::Serialize)]
    struct PidEntryOut<'a> {
        task_id: &'a str,
        pid: u32,
        started_at: &'a str,
    }
    let entries: Vec<PidEntryOut> = kept
        .iter()
        .map(|e| PidEntryOut {
            task_id: e.task_id.as_str(),
            pid: e.pid,
            started_at: e.started_at.as_str(),
        })
        .collect();
    let out = PidFileOut { tasks: entries };
    let json = serde_json::to_string_pretty(&out)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let tmp = workdir.join(format!(".alps-pids.json.prune.{}.tmp", std::process::id()));
    std::fs::write(&tmp, json.as_bytes())?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pidfile_parses_minimal_shape() {
        let body = r#"{"tasks":[{"task_id":"x","pid":42,"started_at":"2026-08-26T10:00:00Z"}]}"#;
        let parsed: PidFile = serde_json::from_str(body).unwrap();
        assert_eq!(parsed.tasks.len(), 1);
        assert_eq!(parsed.tasks[0].task_id, "x");
        assert_eq!(parsed.tasks[0].pid, 42);
        assert_eq!(parsed.tasks[0].started_at, "2026-08-26T10:00:00Z");
    }

    #[test]
    fn pidfile_parses_empty_tasks() {
        let body = r#"{"tasks":[]}"#;
        let parsed: PidFile = serde_json::from_str(body).unwrap();
        assert!(parsed.tasks.is_empty());
    }

    #[test]
    fn pidfile_roundtrip_writer_to_reader() {
        // The writer in run.rs writes a {tasks: [...]} envelope with
        // task_id + pid + started_at. This test pins the schema by
        // round-tripping through both reader + writer shapes.
        #[derive(serde::Serialize)]
        struct PidFileW<'a> {
            tasks: Vec<PidEntryW<'a>>,
        }
        #[derive(serde::Serialize)]
        struct PidEntryW<'a> {
            task_id: &'a str,
            pid: u32,
            started_at: &'a str,
        }
        let writer = PidFileW {
            tasks: vec![PidEntryW {
                task_id: "2026-08-26T100000-aaaaaaaaaaaaaaa",
                pid: 99999,
                started_at: "2026-08-26T10:00:00+00:00",
            }],
        };
        let json = serde_json::to_string(&writer).unwrap();
        let parsed: PidFile = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.tasks[0].task_id, "2026-08-26T100000-aaaaaaaaaaaaaaa");
        assert_eq!(parsed.tasks[0].pid, 99999);
    }
}