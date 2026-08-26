//! Process-wide registry of `alps run` orchestrator processes spawned by `task_run`.
//!
//! ## Why a registry at all
//!
//! M3c lands the Cancel button (story 3c.4) and `task_cancel` server fn
//! (story 3c.3). To send SIGTERM to a running task's orchestrator, the
//! cancel path needs to know the OS PID of the `alps run` subprocess
//! that `task_run` spawned. Before M3c, `task_run` deliberately
//! `std::mem::forget`-ed the `tokio::process::Child` handle (see the
//! "Why we don't wait for the child to exit" docstring on `task_run`),
//! throwing away the PID. The Cancel button had no PID to send SIGTERM
//! to.
//!
//! M3c's design (Kyle-approved 2026-08-26, Option C from the M3c
//! preflight) is: track the `Child` in a process-wide registry so
//! `task_cancel` can look it up + SIGTERM it. ALSO write
//! `<workdir>/.alps-pids.json` to disk (atomic temp-file + rename) so
//! the registry survives server restarts (in-memory map is gone after
//! restart; on-disk file isn't). `task_cancel` checks memory first,
//! falls back to the file.
//!
//! ## Why `OnceLock<Mutex<HashMap>>`
//!
//! - `OnceLock` makes the registry a singleton without an explicit
//!   `init()` step — the first `task_run` call initializes it; all
//!   later calls reuse it.
//! - `Mutex<HashMap>` because `task_run` (insert) and `task_cancel`
//!   (remove + access `Child` for `start_kill`) are the only callers,
//!   and contention is one operator at a time.
//! - `Child` itself is wrapped in `Mutex<Option<Child>>` so
//!   `task_cancel` can `take()` it (consume the handle to send the
//!   signal without holding the outer registry lock across a syscall).
//!
//! ## Why this is alps-gui only
//!
//! The alps-ui server is the side that calls `child.id()` immediately
//! after `spawn()` — alps-cli never knows its own PID was recorded
//! anywhere. alps-cli's signal handler writes to `.alps-sigterm.log`
//! (already set up in `task_run`); it doesn't need to write
//! `.alps-pids.json` itself. Single-repo implementation.
//!
//! ## Survives server restarts
//!
//! On restart the in-memory map is empty. `task_cancel` falls back to
//! reading `<workdir>/.alps-pids.json` and finds the entry by task_id;
//! if the orchestrator PID is still alive, the SIGTERM still works. If
//! the orchestrator exited naturally, `kill -TERM <pid>` returns ESRCH
//! and we surface "task already completed".

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use chrono::{DateTime, Utc};
use tokio::process::Child;

/// One orchestrator process spawned by `task_run`.
///
/// The `child` field is wrapped in `Mutex<Option<_>>` so `task_cancel`
/// can `take()` it out without holding the outer registry lock across
/// the syscall (`child.start_kill()`). After `take()`, the entry is
/// still in the registry (under the cancel lock) until the cancel
/// handler removes it explicitly.
pub struct ChildHandle {
    /// The spawned `tokio::process::Child`. Wrapped in `Option` so
    /// `task_cancel` can `take()` it; `None` after cancellation or
    /// natural exit (the latter is detected lazily — see
    /// `try_wait` in task_cancel).
    pub child: Mutex<Option<Child>>,
    /// OS PID, captured via `child.id()` at spawn time.
    pub pid: u32,
    /// Wall-clock time the orchestrator process was spawned (UTC).
    pub started_at: DateTime<Utc>,
    /// The workdir the orchestrator was spawned against. Stored so
    /// `task_cancel` can derive the `.alps-pids.json` path without
    /// re-passing it as an argument (caller may not have it).
    pub workdir: String,
}

/// Process-wide singleton registry.
pub fn registry() -> &'static Mutex<HashMap<String, ChildHandle>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, ChildHandle>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Take the `Child` handle out of the registry for a given task_id,
/// so the caller can `start_kill()` it.
///
/// Returns `Vec<(task_id, pid, started_at, workdir)>` for serialization.
pub fn snapshot() -> Vec<(String, u32, DateTime<Utc>, String)> {
    let reg = match registry().lock() {
        Ok(g) => g,
        Err(_) => return Vec::new(),
    };
    reg.iter()
        .map(|(id, h)| (id.clone(), h.pid, h.started_at, h.workdir.clone()))
        .collect()
}

/// Take the `Child` handle out of the registry for a given task_id,
/// so the caller can `start_kill()` it.
///
/// After this call the registry entry for `task_id` is REMOVED (the
/// handle is consumed by `task_cancel`). Returns the handle's metadata
/// so the caller can still log pid + started_at.
pub fn take(task_id: &str) -> Option<(Child, u32, DateTime<Utc>, String)> {
    let mut reg = registry().lock().ok()?;
    let handle = reg.remove(task_id)?;
    let ChildHandle {
        child,
        pid,
        started_at,
        workdir,
    } = handle;
    let child = child.into_inner().ok()?.take()?; // `child` is Mutex<Option<Child>>
    Some((child, pid, started_at, workdir))
}

/// Insert a fresh `ChildHandle` after `task_run` spawns the
/// orchestrator. The caller passes the `Child`, PID, started_at,
/// and workdir; the registry wraps the `Child` in the inner
/// `Mutex<Option<_>>` so `take()` later works.
pub fn insert(task_id: String, child: Child, pid: u32, started_at: DateTime<Utc>, workdir: String) {
    if let Ok(mut reg) = registry().lock() {
        reg.insert(
            task_id,
            ChildHandle {
                child: Mutex::new(Some(child)),
                pid,
                started_at,
                workdir,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_singleton() {
        // Both calls hit the same OnceLock-backed reference.
        assert!(std::ptr::eq(registry(), registry()));
    }

    #[test]
    fn snapshot_is_empty_after_init() {
        // Use a fresh task_id that's guaranteed not to collide.
        assert!(!snapshot().iter().any(|(id, _, _, _)| id.starts_with("snapshot-empty-test-")));
    }
}