//! Read-side server functions for the ALPS task list + per-task detail.
//!
//! Both functions shell out to the `alps` CLI's `--json` subcommands
//! (per SPEC §7.1 / US-006 acceptance #3): we go through the CLI rather
//! than `alps_core::persistence::list_tasks` / `read_task` directly so
//! the UI's behavior matches what an operator gets from the terminal —
//! same JSON shape, same exit codes, same workdir guard semantics.
//!
//! ## Function signatures
//!
//! - `tasks_list(workdir: String) -> Result<TaskList, ServerFnError>` —
//!   calls `alps list --json --workdir <workdir>` and returns the
//!   parsed `TaskList`. The `ServerFnError` return type is what the
//!   `#[server]` macro wants for the axum response envelope (per
//!   `MakeAxumError` impl bounds — `String: AsStatusCode + From<ServerFnError>`
//!   isn't satisfied on stable Rust; `ServerFnError` is).
//! - `task_get(workdir: String, task_id: String) -> Result<Option<TaskDetail>, ServerFnError>`
//!   — calls `alps show --json --workdir <workdir> <task_id>`. Returns
//!   `Ok(None)` when the CLI exits with code 2 (not-found, per the
//!   `Command::Show` arm in `alps-cli/src/main.rs:418` `std::process::exit(2)`).
//!
//! ## Why CLI rather than direct `alps_core` calls
//!
//! Acceptance criterion #3 explicitly says: "`tasks_list` and `task_get`
//! shell out to `alps list --json` / `alps show --json` via
//! `Command::new` — they do NOT call `alps_core::persistence::list_tasks`
//! directly". The intent is drift-prevention: any future change to
//! `alps list` (new fields, new sentinel tasks, etc.) shows up in the UI
//! automatically. Going through `alps_core` directly would let the UI
//! and the CLI diverge.
//!
//! ## Why `#[cfg(feature = "server")]` is doubled
//!
//! Acceptance criterion #2 wants the cfg on the function AND the
//! enclosing module. See the module-level docs in `mod.rs` for the full
//! rationale; the short version is that the macro-generated server-side
//! handler would otherwise carry `Command::new` strings into the wasm
//! artifact's dead-code section when compiled without the `server`
//! feature (the macro's own gating is `#[cfg(not(target_arch = "wasm32"))]`,
//! but the symbol-level metadata still ends up in the compilation unit
//! unless we exclude the function entirely).
//!
//! ## Why import the `#[server]` macro from `dioxus_fullstack_macro`
//!
//! The macro is also reachable as `dioxus::fullstack::server` (re-exported
//! through the `fullstack` feature), but `fullstack` and `server` are
//! SEPARATE features on `dioxus` (per `dioxus-0.7.10/Cargo.toml`). US-006's
//! acceptance criterion #6 requires `--features server` to compile cleanly
//! WITHOUT also enabling `fullstack`, so we import the macro from the
//! crate that the `server` feature directly depends on
//! (`dioxus-fullstack-macro` per `dioxus-0.7.10/Cargo.toml:135`'s
//! `dep:dioxus-fullstack-macro` line in the `server` feature). We also
//! add `dioxus-fullstack` and `dioxus-server` as direct deps because the
//! macro's generated code references both `dioxus_fullstack::*` and
//! `dioxus_server::*` paths (see `dioxus-fullstack-macro-0.7.10/src/lib.rs`
//! lines 466-505, 551-556).

#[cfg(feature = "server")] use std::process::Command;

use alps_core::summary::{TaskDetail, TaskList};
use dioxus_fullstack_core::ServerFnError;
use dioxus_fullstack_macro::server;

/// Spawn `alps list --json --workdir <workdir>` and return the parsed
/// `TaskList`.
///
/// ## Errors
///
/// Returns `Err(ServerFnError::ServerError { ... })` (which the macro
/// surfaces to the client as a 500 with the message as the body) when:
/// - The `alps` binary is not on `$PATH` (the `Command::new` call fails
///   with "No such file or directory").
/// - The CLI exits non-zero (a malformed workdir, a permission error,
///   etc.). The error message includes the exit status and stderr tail
///   for diagnostics.
/// - The CLI's stdout does not parse as `TaskList` (drift between the
///   CLI's JSON shape and `alps_core::summary::TaskList` — surfaced
///   loudly rather than silently returning an empty list).
///
/// ## Output contract
///
/// `alps list --json` emits a single `TaskList` JSON document on stdout
/// with shape `{ "workdir": "...", "tasks": [...] }`. We deserialize via
/// `serde_json::from_str` and return it verbatim. No transformation.
#[cfg(feature = "server")]
#[server]
pub async fn tasks_list(workdir: String) -> Result<TaskList, ServerFnError> {
    let output = Command::new("alps")
        .arg("list")
        .arg("--json")
        .arg("--workdir")
        .arg(&workdir)
        .output()
        .map_err(|e| {
            ServerFnError::new(format!(
                "alps list spawn failed (is `alps` on $PATH?): {}",
                e
            ))
        })?;

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ServerFnError::new(format!(
            "alps list exited {}: {}",
            code,
            stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).map_err(|e| {
        ServerFnError::new(format!(
            "alps list stdout did not parse as TaskList: {} (stdout was: {} bytes)",
            e,
            stdout.len()
        ))
    })
}

/// Spawn `alps show --json --workdir <workdir> <task_id>` and return
/// the parsed `TaskDetail` (or `None` if the task doesn't exist).
///
/// ## Not-found semantics
///
/// `alps show <id>` exits with code 2 when the task doesn't exist (see
/// `alps-cli/src/main.rs:418` `std::process::exit(2)` in the `Command::Show`
/// arm). We translate that to `Ok(None)` — the GUI's TaskDetail page
/// uses that to render a 404. The CLI's `TaskNotFound` JSON body (which
/// contains a `suggestion` field) is dropped on the floor here for v1;
/// a follow-up story could surface it as a "did you mean..." hint.
///
/// ## Errors
///
/// Returns `Err(ServerFnError::new(...))` when:
/// - The `alps` binary is not on `$PATH`.
/// - The CLI exits non-zero AND not with code 2 (i.e. a real failure,
///   not a "task not found" miss).
/// - The CLI's stdout does not parse as `TaskDetail`.
#[cfg(feature = "server")]
#[server]
pub async fn task_get(workdir: String, task_id: String) -> Result<Option<TaskDetail>, ServerFnError> {
    let output = Command::new("alps")
        .arg("show")
        .arg("--json")
        .arg("--workdir")
        .arg(&workdir)
        .arg(&task_id)
        .output()
        .map_err(|e| {
            ServerFnError::new(format!(
                "alps show spawn failed (is `alps` on $PATH?): {}",
                e
            ))
        })?;

    // Exit code 2 = "no such task" per alps-cli's `run_show`. Translate
    // to `Ok(None)` so the GUI can render a 404 without a server-error
    // banner. Any other non-zero exit code = real failure.
    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        if code == 2 {
            return Ok(None);
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ServerFnError::new(format!(
            "alps show exited {}: {}",
            code,
            stderr.trim()
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let detail: TaskDetail = serde_json::from_str(&stdout).map_err(|e| {
        ServerFnError::new(format!(
            "alps show stdout did not parse as TaskDetail: {} (stdout was: {} bytes)",
            e,
            stdout.len()
        ))
    })?;

    Ok(Some(detail))
}

// NOTE: no unit tests here — `#[server]`-decorated functions generate
// dual client/server copies and the body is hidden behind `inventory`
// registration; calling them directly from `#[cfg(test)]` modules
// requires the server runtime. The integration smoke in US-007
// (`dx serve` end-to-end + `curl` against `/api/tasks_list`) is the
// load-bearing test for these two functions. A future story that needs
// unit-level coverage should extract the shell-out + parse body into a
// `pub(crate) fn tasks_list_impl(workdir: &str) -> Result<TaskList, ServerFnError>`
// helper behind `#[cfg(feature = "server")] #[cfg(test)]` and call that
// instead.
