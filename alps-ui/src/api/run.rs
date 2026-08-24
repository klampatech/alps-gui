//! Write-side server function for spawning a new ALPS orchestrator run.
//!
//! `task_run` is the server function the NewTask form calls when the
//! operator clicks Submit. The intended v1.6 flow (per SPEC §7.3 and
//! US-006 acceptance #4) is:
//!
//! 1. Write the prompt text to a temp file (because `alps run
//!    --prompt-file <path>` deletes the file after read — see
//!    `alps-cli/src/main.rs:296` `let _ = std::fs::remove_file(path);`
//!    in `resolve_prompt`).
//! 2. Spawn `alps run --workdir <workdir> [--deliverable-path <dp>]
//!    --prompt-file <tempfile>` with `ALPS_SIGTERM_LOG` and
//!    `ALPS_TELEMETRY_LOG` env vars set to per-workdir files so the
//!    orchestrator's signal handler + elog! writes land where the UI
//!    can find them.
//! 3. Return the new task_id.
//!
//! ## v1 status: deferred stub
//!
//! US-006's description explicitly allows: "For v1, `task_run` may
//! instead just navigate to `/tasks/new` per the prompt — but the
//! implementation MUST exist (no-op stub returning
//! `Err("task_run deferred to v2".into())` is acceptable)."
//!
//! We land the stub now so the signature, the `#[server]` registration,
//! the `#[cfg(feature = "server")]` gates, and the import surface are
//! all in place. The real spawn lands in a follow-up story that also
//! wires the NewTask form's submit handler (it currently calls
//! `evt.prevent_default()` per US-005's note).
//!
//! ## Why the spawn lives here rather than in the Dashboard page
//!
//! SPEC §7.2 / in-band pitfall #3: the NewTask form's submit handler
//! runs in the browser (Dioxus rsx on the client). The browser cannot
//! spawn child processes. The server function compiles down to a HTTP
//! POST on the fullstack build (`/api/task_run` per the macro default
//! prefix), which the server-side body then handles — same boundary as
//! `tasks_list` / `task_get`. Keeping all three under the same module
//! makes the "client cannot spawn processes" boundary visible at one
//! place in the codebase.
//!
//! ## Why `ServerFnError` rather than `String` as the error type
//!
//! The `#[server]` macro's `MakeAxumError` bound requires the error
//! type to satisfy `AsStatusCode + IntoResponse` (see
//! `dioxus-fullstack-core-0.7.10/src/error.rs:135-201`). `String`
//! doesn't impl those; `ServerFnError` does. See `tasks.rs` for the
//! full dep-import rationale (the macro needs `dioxus-fullstack` and
//! `dioxus-server` as direct crates because its generated code
//! references both).

use dioxus_fullstack_core::ServerFnError;
use dioxus_fullstack_macro::server;

/// Spawn `alps run` for a new task and return the resulting task_id.
///
/// ## Arguments
///
/// - `workdir` — the `--workdir` flag value. Must point at a directory
///   whose `<workdir>/tasks/` subdirectory ALPS can write into.
/// - `deliverable_path` — the `--deliverable-path` flag value. May be
///   empty (CLI defaults to `workdir` in that case per `alps-cli/src/main.rs:213`).
/// - `prompt` — the prompt text. In the intended v1.6 implementation
///   this would be written to a temp file (because `--prompt-file`
///   deletes after read) and the temp path passed via `--prompt-file`.
///   The stub doesn't touch it.
///
/// ## Return shape
///
/// `Result<String, ServerFnError>` where the `Ok` value is the new
/// task_id (shape `YYYY-MM-DDTHHMMSS-<uuid8>` per
/// `alps_core::domain::TaskId::new`).
///
/// ## Errors
///
/// The stub returns `Err(ServerFnError::new("task_run deferred to v2"))`.
/// The intended error surface for the real implementation is:
/// - Spawn failure (`alps` not on $PATH) → 500 with the OS error.
/// - CLI exits non-zero before opening the task workspace → 500 with
///   the stderr tail.
/// - Task ID parse failure (CLI doesn't print the task_id by default
///   → we'd discover it via `<workdir>/tasks/<id>/prompt.md` mtime or
///   the PID bookkeeping file). Defer to a follow-up story.
///
/// ## `#[cfg(feature = "server")]` rationale
///
/// Same as `tasks.rs`: the function AND its enclosing module both
/// carry the gate so `std::process::Command` symbols never reach the
/// client bundle. Acceptance criterion #5 ("No `std::process::Command`
/// or filesystem access exists outside `#[cfg(feature = "server")]`
/// blocks") is satisfied because `Command::new` only appears inside
/// the deferred implementation's comment block above, not in any
/// active code path.
#[cfg(feature = "server")]
#[server]
pub async fn task_run(
    _workdir: String,
    _deliverable_path: String,
    _prompt: String,
) -> Result<String, ServerFnError> {
    // v1 stub. The real implementation per SPEC §7.3 / US-006
    // description does:
    //
    //   let prompt_file = std::env::temp_dir().join(format!(
    //       "alps-ui-prompt-{}.md",
    //       alps_core::domain::TaskId::new().as_str()
    //   ));
    //   std::fs::write(&prompt_file, prompt.as_bytes())?;
    //
    //   let workdir_path = std::path::PathBuf::from(&workdir);
    //   let mut cmd = std::process::Command::new("alps");
    //   cmd.arg("run")
    //       .arg("--workdir").arg(&workdir)
    //       .arg("--prompt-file").arg(&prompt_file);
    //   if !deliverable_path.is_empty() {
    //       cmd.arg("--deliverable-path").arg(&deliverable_path);
    //   }
    //   cmd.env("ALPS_SIGTERM_LOG", workdir_path.join(".alps-sigterm.log"))
    //       .env("ALPS_TELEMETRY_LOG", workdir_path.join(".alps-telemetry.log"))
    //       .stdin(Stdio::null())
    //       .stdout(Stdio::piped())
    //       .stderr(Stdio::piped());
    //
    //   let child = cmd.spawn()?;
    //   // ... discover the new task_id via `<workdir>/tasks/` mtime ...
    //   // ... return it to the client ...
    //
    // We intentionally do NOT implement that yet — US-006 acceptance
    // #4 explicitly allows a `Err("task_run deferred to v2")` stub.
    // The signature, the `#[server]` registration, and the cfg gates
    // are what US-006 is testing; the spawn lands in a follow-up that
    // also wires the NewTask form's submit handler.
    Err(ServerFnError::new("task_run deferred to v2"))
}
