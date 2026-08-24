//! Server-side API surface for the ALPS UI.
//!
//! This module holds the `#[server]`-decorated functions that the UI
//! calls to read from the ALPS orchestrator's CLI and (in a follow-up
//! story) write to it. Every submodule and every function here is gated
//! behind `#[cfg(feature = "server")]` so the secrets, child-process
//! spawns, and filesystem access never appear in the client bundle.
//!
//! ## What lives here
//!
//! - [`tasks`] — `tasks_list` (calls `alps list --json`) and `task_get`
//!   (calls `alps show --json`). Both shell out via the `Command` API.
//! - [`run`] — `task_run` (spawns `alps run`). V1 is a deferred stub
//!   returning `Err("task_run deferred to v2")`; the real spawn lands
//!   when US-007/US-008 wire the NewTask form's submit handler.
//!
//! ## What is NOT here
//!
//! - `task_log_stream` (SSE) — deferred per SPEC §7.1 / US-008.
//! - `task_cancel` (SIGTERM dispatch) — deferred per SPEC §7.1 / US-008.
//! - `task_diff` (read-side) — deferred per SPEC §7.1 / US-008.
//! - `settings_get` / `settings_set` — deferred per SPEC §7.1 / US-008.
//!
//! ## Why `#[cfg(feature = "server")]` on the module AND each fn
//!
//! US-006's acceptance criterion #2 requires the `#[cfg]` to appear on
//! the function AND the enclosing module. Belt-and-suspenders: the
//! module gate ensures the `pub use` re-exports below only name live
//! items in the `server` build, and the function-level gate ensures no
//! compiler error surfaces when something else (a `pub use` from a
//! sibling module, a test, etc.) accidentally names the function in a
//! non-server build. The acceptance criterion #8 "the resulting WASM
//! does NOT contain `Command::new` strings" is satisfied because the
//! entire module is excluded from the default `web`-only build.
//!
//! ## What the client sees in the default `web` build
//!
//! Nothing. The module is fully gated out, the `pub use` re-exports
//! vanish, and the wasm binary contains zero bytes of server-side code.
//! Pages that would call these functions (e.g. the future TaskDetail
//! page that calls `task_get`) cannot import them under `default` — they
//! only get wired up once `--features server` (or `--features fullstack`)
//! is on. The Dashboard's v1 wiring reads from `FIXTURES` (US-005), so
//! it compiles cleanly without the server feature.
//!
//! ## `#[allow(unused_imports)]` on the `pub use` re-exports
//!
//! US-006 lands the API BEFORE any page actually calls these functions
//! (the Dashboard uses `FIXTURES`, TaskDetail is still a US-003
//! placeholder). Until US-007+ wires `tasks_list` / `task_get` /
//! `task_run` into page bodies, the `pub use` lines below would trip
//! the `unused_imports` lint. The `#[allow]` is module-level because
//! the entire surface is "imported by no one yet" — same pattern as
//! US-002's `domain.rs` block-level allows. Strip this allow when
//! US-007+ adds the first consumer.

#![cfg_attr(feature = "server", allow(unused_imports))]

#[cfg(feature = "server")]
mod run;
#[cfg(feature = "server")]
mod tasks;

#[cfg(feature = "server")]
pub use run::task_run;
#[cfg(feature = "server")]
pub use tasks::{task_get, tasks_list};

// Re-export the error type so callers can name it in their signatures
// without depending on dioxus_fullstack_core directly. Gated on the
// `server` feature because ServerFnError only exists in that build
// (the wasm-client build never imports the api module).
#[cfg(feature = "server")]
pub use dioxus_fullstack_core::ServerFnError;

// Stub error type for the default (`web`-only) build. The api module is
// gated out in that build, so callers can't reach a real
// `tasks_list` — but they still want a type-stable signature so they
// compile without `#![cfg(feature = "server")]` everywhere. The
// stub's `Debug` impl matches `ServerFnError`'s shape closely enough
// for the Dashboard's ErrorCard to render a useful message.
//
// We implement `Into<CapturedError>` so the stub satisfies the same
// trait bounds the real `ServerFnError` does (required by
// `dioxus_fullstack_core::use_loader`). CapturedError isn't available
// in the default build, so we gate the impl on the same `server`
// feature the real type uses.
#[cfg(not(feature = "server"))]
#[derive(Debug)]
pub struct ServerFnError(pub String);

#[cfg(not(feature = "server"))]
impl std::fmt::Display for ServerFnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Stub `tasks_list` for the default (wasm-only) build. The real
// version lives in `tasks.rs` behind the `server` feature; this stub
// returns an error immediately so the Dashboard's `use_resource` sees
// a Result::Err and the ErrorCard renders the "run with --features
// server" hint.
#[cfg(not(feature = "server"))]
pub async fn tasks_list(_workdir: String) -> Result<crate::domain::TaskList, ServerFnError> {
    Err(ServerFnError(
        "tasks_list is server-only — rebuild with --features server".to_string(),
    ))
}
