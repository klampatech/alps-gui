//! Write-side server function for spawning a new ALPS orchestrator run.
//!
//! `task_run` is the server function the NewTask form calls when the
//! operator clicks Submit. The M2 flow (per SPEC §7.3 and US-006
//! acceptance #4) is:
//!
//! 1. Write the prompt text to a temp file (because `alps run
//!    --prompt-file <path>` deletes the file after read — see
//!    `alps-cli/src/main.rs:296` `let _ = <fs>::remove_file(path);`
//!    in `resolve_prompt`).
//! 2. Spawn `alps run --workdir <workdir> [--deliverable-path <dp>]
//!    --prompt-file <tempfile>` with `ALPS_SIGTERM_LOG` and
//!    `ALPS_TELEMETRY_LOG` env vars set to per-workdir files so the
//!    orchestrator's signal handler + elog! writes land where the UI
//!    can find them.
//! 3. Discover the new task_id by parsing stdout for the
//!    `task_id=<id>` line that `tracing::info!` emits immediately
//!    on startup (verified 2026-08-24: alps's
//!    `tracing_subscriber::fmt::init()` defaults to stdout).
//! 4. Return the task_id to the client.
//!
//! ## Why `#[cfg(feature = "server")]` is doubled
//!
//! Same as `tasks.rs`: the function AND its enclosing module both
//! carry the gate so `Command` symbols never reach the client bundle.
//! US-006 acceptance criterion #5 ("No `Command::new` or filesystem
//! access exists outside `#[cfg(feature = "server")]` blocks") is
//! satisfied because the only `Command::new` is inside the
//! `#[cfg(feature = "server")]` body below.
//!
//! ## Why stdout parsing for task_id discovery
//!
//! `alps run` generates `task_id` at startup via
//! `alps_core::domain::TaskId::new()` (a `YYYY-MM-DDTHHMMSS-<uuid>`
//! string built from `Utc::now()` + a fresh v4 UUID — neither
//! predictable from the parent process). Three options for discovery:
//!
//! 1. Poll `<workdir>/tasks/` for a freshly-created directory (mtime).
//!    — Works but racy with subsequent tasks created in the same
//!    second, and requires a loop with a small sleep.
//! 2. Mtime-poll for a directory newer than the spawn time.
//!    — Same issue, less flaky but still loop-dependent.
//! 3. Parse stdout for `task_id=<id>`. The CLI logs
//!    `INFO alps.cli: starting task task_id=<id>` via `tracing::info!`
//!    within the first ~5 ms of startup. `tracing-subscriber`'s
//!    default writer is stdout, so this lands on stdout — not stderr
//!    (the alps CLI uses `eprintln!` for its own `[alps-diag]`
//!    lines but `tracing::info!` for structured fields).
//!
//! Option 3 is the cleanest: read stdout to EOF after the child
//! exits or buffer to first task_id line, whichever comes first.
//! Since we spawn detached (`Stdio::null()` for stdin, piped stderr)
//! and only need to return the task_id, we just read stderr until we
//! find a `task_id=` line or hit EOF (whichever comes first), then
//! let the child continue running in the background. Killing the
//! parent before the child reads task_id is the only race; the
//! fix is to wait for the first `task_id=` match with a bounded
//! timeout (5 seconds) and treat timeout as a spawn failure.
//!
//! ## Why we don't wait for the child to exit
//!
//! `alps run` is the orchestrator — it runs until the task is done
//! (could be minutes to hours). We spawn it, capture the task_id,
//! return to the client immediately, and let the child run in the
//! background. The UI polls `tasks_list` to see state transitions.
//! This matches the smoke-A.txt §Topic A.6 contract: the NewTask form
//! "navigates to /tasks/<id>" once submitted (deferred to M3) —
//! returning the task_id gives the client everything it needs to
//! do that navigation without waiting for the child.
//!
//! ## Why we set `ALPS_SIGTERM_LOG` / `ALPS_TELEMETRY_LOG` env vars
//!
//! US-006 acceptance criterion #4 requires these. The CLI's signal
//! handlers (see `alps-cli/src/main.rs:130` setup) write a marker +
//! backtrace to the SIGTERM log when the orchestrator is killed.
//! The elog! macro (see `alps-core/src/telemetry.rs:150`) writes
//! per-task telemetry to the TELEMETRY log when `ALPS_TELEMETRY_LOG`
//! is set. M3 (TaskLog page) reads the TELEMETRY log to render the
//! live activity stream; M3 (Cancel button) sends SIGTERM and reads
//! the SIGTERM log to render the kill receipt.

use std::process::Stdio;

use dioxus_fullstack_core::ServerFnError;
use dioxus_fullstack_macro::server;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Spawn `alps run` for a new task and return the resulting task_id.
///
/// ## Arguments
///
/// - `workdir` — the `--workdir` flag value. Must point at a directory
///   whose `<workdir>/tasks/` subdirectory ALPS can write into.
/// - `deliverable_path` — the `--deliverable-path` flag value. May be
///   empty (CLI defaults to the workdir's nested ralph git per
///   `alps-cli/src/main.rs:813-830`).
/// - `prompt` — the prompt text. Written to a temp file because
///   `--prompt-file` deletes after read.
///
/// ## Return shape
///
/// `Result<String, ServerFnError>` where the `Ok` value is the new
/// task_id (shape `YYYY-MM-DDTHHMMSS-<uuid8>` per
/// `alps_core::domain::TaskId::new`).
///
/// ## Errors
///
/// - Spawn failure (`alps` not on $PATH) → 500 with the OS error.
/// - Temp file write failure → 500 with the OS error.
/// - Child exits before printing the `task_id=` line (very rare —
///   would require the CLI to error out in <5ms) → 500 with
///   "task_run: child exited before printing task_id".
/// - Stderr parse timeout (5s) → 500 with "task_run: timed out
///   waiting for task_id on stderr".
#[cfg(feature = "server")]
#[server]
pub async fn task_run(
    workdir: String,
    deliverable_path: String,
    prompt: String,
) -> Result<String, ServerFnError> {
    let workdir_path = std::path::PathBuf::from(&workdir);

    // Step 1: write prompt to a temp file. The CLI deletes the
    // `--prompt-file` after read (`alps-cli/src/main.rs:296`), so we
    // have to use a fresh path every call — the PID is the cleanest
    // unique-enough suffix.
    let pid = std::process::id();
    let prompt_file = std::env::temp_dir().join(format!("alps-ui-prompt-{pid}.md"));
    if let Err(e) = std::fs::write(&prompt_file, prompt.as_bytes()) {
        return Err(ServerFnError::new(format!(
            "task_run: failed to write prompt file {}: {}",
            prompt_file.display(),
            e
        )));
    }

    // Step 2: assemble the spawn command.
    let mut cmd = Command::new("alps");
    cmd.arg("run")
        .arg("--workdir")
        .arg(&workdir)
        .arg("--prompt-file")
        .arg(&prompt_file);
    if !deliverable_path.trim().is_empty() {
        cmd.arg("--deliverable-path").arg(&deliverable_path);
    }

    // Per SPEC §7.3 / US-006 acceptance #4: set the per-workdir
    // signal + telemetry log paths. The CLI's elog! macro writes
    // here; M3 (Cancel button) reads SIGTERM_LOG after sending
    // SIGTERM to surface the kill receipt.
    cmd.env(
        "ALPS_SIGTERM_LOG",
        workdir_path.join(".alps-sigterm.log"),
    )
    .env(
        "ALPS_TELEMETRY_LOG",
        workdir_path.join(".alps-telemetry.log"),
    )
    // The `tracing::info!("starting task task_id=...")` line lands on
    // stdout (tracing-subscriber's default writer), NOT stderr.
    // `elog!` calls land on stderr. We need the tracing line to
    // discover the task_id, so pipe stdout and read it. Stderr is
    // also piped to prevent the child from blocking on a full pipe
    // buffer.
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    // Step 3: spawn. The alps-ui server build has `tokio = "full"`
    // available via the alps-core sibling dep (gated to native
    // targets).
    let mut child = cmd.spawn().map_err(|e| {
        ServerFnError::new(format!(
            "task_run: spawn failed (is `alps` on $PATH?): {e}"
        ))
    })?;

    // Capture the OS PID immediately. Before M3c, we `mem::forget`-
    // ed the Child and lost the PID, which made the Cancel button
    // (story 3c.4) impossible — there was no PID to SIGTERM. M3c
    // (2026-08-26, Kyle-approved Option C) inserts the Child into
    // the process registry so `task_cancel` can find + signal it,
    // and ALSO writes `<workdir>/.alps-pids.json` so the tracking
    // survives an alps-ui server restart.
    //
    // `Child::id()` returns `Option<u32>` — None on some platforms if
    // the child hasn't been fully spawned yet. In practice on Linux
    // it's always Some by the time spawn() returns, but be defensive.
    let pid = child.id().unwrap_or_else(|| std::process::id());
    let started_at = chrono::Utc::now();

    // Step 4: read stdout until we see `task_id=<id>` (or 5s
    // timeout, or EOF). The CLI emits this line via tracing within
    // the first few ms of startup.
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ServerFnError::new("task_run: no stdout handle"))?;

    let task_id = match read_task_id_from_stdout(stdout).await {
        Ok(id) => id,
        Err(e) => {
            // Best-effort kill so we don't leave a half-started
            // orchestrator lying around if we couldn't discover its
            // task_id.
            let _ = child.start_kill();
            return Err(ServerFnError::new(format!("task_run: {e}")));
        }
    };

    // M3c: register the Child + write `.alps-pids.json` so
    // `task_cancel` can find this orchestrator later. The registry
    // insert is infallible (just an Arc<Mutex> bump); the file
    // write is the load-bearing piece — if it fails (disk full,
    // permission denied), log a warning but still return the
    // task_id. The in-memory map covers the same-task-server path;
    // the file is the cross-restart fallback.
    crate::api::process_registry::insert(
        task_id.clone(),
        child,
        pid,
        started_at,
        workdir_path.to_string_lossy().to_string(),
    );
    if let Err(e) = write_alps_pids_json(&workdir_path) {
        eprintln!(
            "task_run: warning — failed to write {}/.alps-pids.json: {e}",
            workdir_path.display()
        );
    }

    Ok(task_id)
}

/// Serialize the current registry snapshot to
/// `<workdir>/.alps-pids.json` atomically (temp file + rename).
///
/// File shape:
/// ```json
/// {
///   "tasks": [
///     {"task_id": "...", "pid": 12345, "started_at": "2026-08-26T..."},
///     ...
///   ]
/// }
/// ```
///
/// Atomic write so a partial write never lands in the file (a
/// partial write would let `task_cancel` see a missing entry mid-
/// transition and surface a confusing "no such task" error).
fn write_alps_pids_json(workdir: &std::path::Path) -> std::io::Result<()> {
    use serde::Serialize;

    #[derive(Serialize)]
    struct PidFile<'a> {
        tasks: Vec<PidEntry<'a>>,
    }
    #[derive(Serialize)]
    struct PidEntry<'a> {
        task_id: &'a str,
        pid: u32,
        started_at: String,
    }

    let snapshot = crate::api::process_registry::snapshot();
    let entries: Vec<PidEntry> = snapshot
        .iter()
        .map(|(id, pid, started_at, _workdir)| PidEntry {
            task_id: id.as_str(),
            pid: *pid,
            started_at: started_at.to_rfc3339(),
        })
        .collect();

    let file = PidFile { tasks: entries };
    let json = serde_json::to_string_pretty(&file)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let target = workdir.join(".alps-pids.json");
    // Temp file in the same directory so the rename is atomic
    // (same filesystem).
    let pid = std::process::id();
    let tmp = workdir.join(format!(".alps-pids.json.{pid}.tmp"));
    std::fs::write(&tmp, json.as_bytes())?;
    std::fs::rename(&tmp, &target)?;
    Ok(())
}

/// Read stdout line-by-line, return the first match for
/// `task_id=(\d{4}-\d{2}-\d{2}T\d{6}-[0-9a-f]+)`. Bounded at 5s.
///
/// `tracing::info!("starting task task_id=<id>")` writes to stdout
/// via `tracing-subscriber::fmt::init()` (the alps CLI's default
/// setup — see `alps-cli/src/main.rs:140`).
///
/// Errors:
/// - `timeout` — 5s elapsed with no match.
/// - `eof` — child closed stdout before printing.
/// - `io` — read error.
async fn read_task_id_from_stdout(
    stdout: tokio::process::ChildStdout,
) -> Result<String, String> {
    let mut reader = BufReader::new(stdout).lines();
    let timeout = std::time::Duration::from_secs(5);

    let outcome = tokio::time::timeout(timeout, async {
        while let Some(line) = reader.next_line().await.map_err(|e| {
            format!("io: read stdout line: {e}")
        })? {
            if let Some(id) = find_task_id(&line) {
                return Ok::<_, String>(id);
            }
        }
        Err("eof: child closed stdout before printing task_id".to_string())
    })
    .await;

    match outcome {
        Ok(Ok(id)) => Ok(id),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("timeout: timed out waiting for task_id on stdout".to_string()),
    }
}

/// Find `task_id=<id>` in a stdout line.
///
/// tracing-subscriber's default fmt writer emits ANSI color codes
/// when stdout is a TTY (and `dx serve`'s stdio is a pty, so this
/// fires in our function-test path). The colored form looks like:
///
///   task_id\u{1b}[0m\u{1b}[2m=\u{1b}[0m2026-08-25T00:44:56-...
///
/// We strip the ANSI escape sequences between `task_id` and `=`
/// (and accept them between `=` and the id) so the match works in
/// both colored + plain output.
///
/// The TaskId shape is `YYYY-MM-DDTHHMMSS-<hex>` (see
/// `alps_core::domain::TaskId::new()`).
fn find_task_id(line: &str) -> Option<String> {
    let needle = "task_id";
    let start = line.find(needle)? + needle.len();
    let rest = &line[start..];

    // Skip ANSI escapes between "task_id" and "=". An escape
    // sequence is ESC '[' <params> <final-byte> where final-byte is
    // in 0x40..=0x7e (per ECMA-48 §5.4).
    let bytes = rest.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            // Walk forward until we hit a final byte.
            i += 2;
            while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                i += 1;
            }
            if i < bytes.len() {
                i += 1; // consume the final byte
            }
        } else {
            break;
        }
    }
    // Now expect '=' (possibly followed by more ANSI codes).
    if bytes.get(i) != Some(&b'=') {
        return None;
    }
    i += 1;
    // Skip any ANSI codes after the '='.
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            i += 2;
            while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                i += 1;
            }
            if i < bytes.len() {
                i += 1;
            }
        } else {
            break;
        }
    }

    // Walk the TaskId shape.
    let tail = &rest[i..];
    let tb = tail.as_bytes();
    let mut j = 0;
    if j + 4 > tb.len() || !tb[j..j + 4].iter().all(|b| b.is_ascii_digit()) {
        return None;
    }
    j += 4;
    if tb.get(j) != Some(&b'-') {
        return None;
    }
    j += 1;
    if j + 2 > tb.len() || !tb[j..j + 2].iter().all(|b| b.is_ascii_digit()) {
        return None;
    }
    j += 2;
    if tb.get(j) != Some(&b'-') {
        return None;
    }
    j += 1;
    if j + 2 > tb.len() || !tb[j..j + 2].iter().all(|b| b.is_ascii_digit()) {
        return None;
    }
    j += 2;
    if tb.get(j) != Some(&b'T') {
        return None;
    }
    j += 1;
    if j + 6 > tb.len() || !tb[j..j + 6].iter().all(|b| b.is_ascii_digit()) {
        return None;
    }
    j += 6;
    if tb.get(j) != Some(&b'-') {
        return None;
    }
    j += 1;
    let hex_start = j;
    while j < tb.len() && (tb[j].is_ascii_digit() || (b'a'..=b'f').contains(&tb[j])) {
        j += 1;
    }
    if j == hex_start {
        return None;
    }
    Some(tail[..j].to_string())
}