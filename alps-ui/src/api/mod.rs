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
//! - [`log`] — `task_log_tail_telemetry` (reads `<workdir>/.alps-telemetry.log`)
//!   and `task_log_tail_ralph` (reads `<workdir>/tasks/<id>/implementation/ralph/.ralph-stderr.log`).
//!   Pure read-side, polled-tail design (M3b).
//!
//! ## What is NOT here
//!
//! - `task_log_stream` (SSE) — deferred to v2. M3b uses polled tails.
//! - `task_cancel` (SIGTERM dispatch) — M3c.
//! - `task_diff` (read-side) — M3c.
//! - `settings_get` / `settings_set` — M4/M5.
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
pub mod cancel;
#[cfg(feature = "server")]
pub mod diff;
#[cfg(feature = "server")]
pub mod log;
#[cfg(feature = "server")]
pub mod process_registry;
#[cfg(feature = "server")]
pub mod run;
#[cfg(feature = "server")]
pub mod tasks;
#[cfg(feature = "server")]
pub mod workdir;

#[cfg(feature = "server")]
pub use cancel::task_cancel;
#[cfg(feature = "server")]
pub use diff::{task_diff, CommitDiff};
#[cfg(feature = "server")]
pub use log::{task_log_tail_ralph, task_log_tail_telemetry, LogLine};
#[cfg(feature = "server")]
pub use run::task_run;
#[cfg(feature = "server")]
pub use tasks::{task_get, tasks_list};
#[cfg(feature = "server")]
pub use workdir::{get_workdir, set_workdir};

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
#[cfg(not(feature = "server"))]
#[derive(Debug)]
pub struct ServerFnError(pub String);

#[cfg(not(feature = "server"))]
impl std::fmt::Display for ServerFnError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Stub `tasks_list` for the wasm (browser) build.
//
// The real version lives in `tasks.rs` behind the `server` feature
// and uses the `#[server]` proc-macro, which generates two paths:
//   - `server` feature ON: the body runs in-process (axum dispatch).
//   - `server` feature OFF: a stub that should HTTP POST to
//     `/api/<name>` via web-sys fetch.
//
// On native (non-wasm) builds we still use the macro-generated
// dispatch via `dioxus-fullstack`. On wasm builds we can't pull
// `dioxus-fullstack` (it transitively brings reqwest → tokio → mio,
// none of which compile to wasm32-unknown-unknown), so this stub
// does the POST by hand using web-sys fetch.
//
// ## Why module_path!() is hardcoded
//
// The endpoint path is hashed by the `#[server]` macro from
// `CARGO_MANIFEST_DIR:module_path!()` using xxh64. The macro
// computes `module_path!()` from inside `tasks.rs` (the file
// holding the `#[server]`-decorated function), so its value is
// `alps_ui::api::tasks`. The wasm stub lives in `api/mod.rs`,
// so its local `module_path!()` would be `alps_ui::api` — wrong.
// Hardcode the macro's `module_path!()` value here so the wasm
// hash matches the server hash exactly. If `tasks_list` ever moves
// to a different module, this string must move too.
#[cfg(all(target_arch = "wasm32", not(feature = "server")))]
async fn wasm_post_json<T: serde::Serialize, R: serde::de::DeserializeOwned>(
    module: &str,
    fn_name: &str,
    args: &T,
) -> Result<R, ServerFnError> {
    use serde::Serialize;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::JsValue;
    use wasm_bindgen_futures::JsFuture;

    let body = serde_json::to_string(args)
        .map_err(|e| ServerFnError(format!("serialize args: {e}")))?;

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_body(&JsValue::from_str(&body));

    // The endpoint path is hashed by the `#[server]` macro from
    // `CARGO_MANIFEST_DIR:module_path!()` using xxh64. The macro
    // computes `module_path!()` from inside the file holding the
    // `#[server]`-decorated function (so its value is
    // `alps_ui::api::<module>` where <module> is `tasks`, `run`,
    // etc.). The wasm stubs live in `api/mod.rs`, so their local
    // `module_path!()` would be `alps_ui::api` — wrong. Hardcode
    // the macro's `module_path!()` value here so the wasm hash
    // matches the server hash exactly.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let hash_input = format!("{manifest_dir}:alps_ui::api::{module}");
    let hash = xxhash_rust::const_xxh64::xxh64(hash_input.as_bytes(), 0);
    let url = format!("/api/{fn_name}{hash}");
    web_sys::console::log_1(&JsValue::from_str(&format!("[alps-ui] POST {url}")));

    let request = web_sys::Request::new_with_str_and_init(&url, &opts)
        .map_err(|e| ServerFnError(format!("create request: {e:?}")))?;
    request
        .headers()
        .set("Content-Type", "application/json")
        .map_err(|e| ServerFnError(format!("set header: {e:?}")))?;

    let window = web_sys::window().ok_or_else(|| ServerFnError("no window".to_string()))?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| ServerFnError(format!("fetch: {e:?}")))?;
    let response: web_sys::Response = resp_value.unchecked_into();

    if !response.ok() {
        let status = response.status();
        return Err(ServerFnError(format!("HTTP {status}")));
    }

    let text_promise = response
        .text()
        .map_err(|e| ServerFnError(format!("text(): {e:?}")))?;
    let text_value = JsFuture::from(text_promise)
        .await
        .map_err(|e| ServerFnError(format!("text await: {e:?}")))?;
    let text: String = text_value
        .as_string()
        .ok_or_else(|| ServerFnError("response not a string".to_string()))?;

    serde_json::from_str(&text)
        .map_err(|e| ServerFnError(format!("parse response: {e}; body={text}")))
}

#[cfg(all(target_arch = "wasm32", not(feature = "server")))]
pub async fn tasks_list(workdir: String) -> Result<crate::domain::TaskList, ServerFnError> {
    #[derive(serde::Serialize)]
    struct Args {
        workdir: String,
    }
    wasm_post_json("tasks", "tasks_list", &Args { workdir }).await
}

#[cfg(all(target_arch = "wasm32", not(feature = "server")))]
pub async fn task_run(
    workdir: String,
    deliverable_path: String,
    prompt: String,
) -> Result<String, ServerFnError> {
    #[derive(serde::Serialize)]
    struct Args {
        workdir: String,
        deliverable_path: String,
        prompt: String,
    }
    wasm_post_json("run", "task_run", &Args { workdir, deliverable_path, prompt }).await
}

// Wasm-side stub for `task_get` — the canonical 2-arg body shape
// `{workdir, task_id}` matches the `#[server]` macro's
// `___Body_Serialize__<T0, T1>` generation per M3a pitfalls note.
// The server fn lives in `tasks.rs`; the macro's `module_path!()` is
// therefore `alps_ui::api::tasks`, which is what we pass as the
// helper's `module` argument.
#[cfg(all(target_arch = "wasm32", not(feature = "server")))]
pub async fn task_get(workdir: String, task_id: String) -> Result<Option<crate::domain::TaskDetail>, ServerFnError> {
    #[derive(serde::Serialize)]
    struct Args {
        workdir: String,
        task_id: String,
    }
    wasm_post_json("tasks", "task_get", &Args { workdir, task_id }).await
}

// Wasm-side stubs for the M3b log-tail endpoints. The server fns live
// in `log.rs`; the macro's `module_path!()` for both is therefore
// `alps_ui::api::log`, which is the helper's `module` argument.
//
// The body shape matches `___Body_Serialize__<workdir, since_line_no>`
// and `___Body_Serialize__<workdir, task_id, since_line_no>` per the
// `#[server]` macro's arg-count → body struct mapping.
#[cfg(all(target_arch = "wasm32", not(feature = "server")))]
pub async fn task_log_tail_telemetry(
    workdir: String,
    since_line_no: u64,
) -> Result<Vec<LogLine>, ServerFnError> {
    #[derive(serde::Serialize)]
    struct Args {
        workdir: String,
        since_line_no: u64,
    }
    wasm_post_json(
        "log",
        "task_log_tail_telemetry",
        &Args { workdir, since_line_no },
    )
    .await
}

#[cfg(all(target_arch = "wasm32", not(feature = "server")))]
pub async fn task_log_tail_ralph(
    workdir: String,
    task_id: String,
    since_line_no: u64,
) -> Result<Vec<LogLine>, ServerFnError> {
    #[derive(serde::Serialize)]
    struct Args {
        workdir: String,
        task_id: String,
        since_line_no: u64,
    }
    wasm_post_json(
        "log",
        "task_log_tail_ralph",
        &Args { workdir, task_id, since_line_no },
    )
    .await
}

// Wasm stub for `task_cancel` (M3c, story 3c.3). Same wasm_post_json
// pattern as the other server fns. The real implementation lives in
// `api/cancel.rs` (gated on `feature = "server"`).
#[cfg(all(target_arch = "wasm32", not(feature = "server")))]
pub async fn task_cancel(
    workdir: String,
    task_id: String,
) -> Result<(), ServerFnError> {
    #[derive(serde::Serialize)]
    struct Args {
        workdir: String,
        task_id: String,
    }
    wasm_post_json("cancel", "task_cancel", &Args { workdir, task_id }).await
}

// Wasm stub for `task_diff` (M3c, story 3c.1). Same wasm_post_json
// pattern. Uses the local `CommitDiff` stub (also
// `cfg(not(feature = "server"))`) so this type-checks in the wasm
// build where the real `api::diff` module is gated out.
#[cfg(all(target_arch = "wasm32", not(feature = "server")))]
pub async fn task_diff(
    workdir: String,
    task_id: String,
) -> Result<Vec<CommitDiff>, ServerFnError> {
    #[derive(serde::Serialize)]
    struct Args {
        workdir: String,
        task_id: String,
    }
    wasm_post_json(
        "diff",
        "task_diff",
        &Args { workdir, task_id },
    )
    .await
}

// Wasm stub for `get_workdir` (M4-proper). No args — just hits
// `/api/get_workdir<hash>` with an empty JSON object.
#[cfg(all(target_arch = "wasm32", not(feature = "server")))]
pub async fn get_workdir() -> Result<String, ServerFnError> {
    #[derive(serde::Serialize)]
    struct Args {}
    wasm_post_json("workdir", "get_workdir", &Args {}).await
}

// Wasm stub for `set_workdir` (M4-proper). Single-arg: the new
// workdir path.
#[cfg(all(target_arch = "wasm32", not(feature = "server")))]
pub async fn set_workdir(path: String) -> Result<(), ServerFnError> {
    #[derive(serde::Serialize)]
    struct Args {
        path: String,
    }
    wasm_post_json("workdir", "set_workdir", &Args { path }).await
}

// Stub `LogLine` type for the default (`web`-only) build, mirroring
// the `ServerFnError` stub above. The `api::log` module is gated out
// in this build, so the wasm/native stubs above can't reach the real
// `LogLine` type — but they still want a type-stable return signature
// so callers compile without `#![cfg(feature = "server")]` everywhere.
// Field shape matches the real `LogLine` (see `api/log.rs`).
//
// The `new()` constructor is required because unit tests in
// `pages/task_log.rs` reference `LogLine::new(n, text)` to construct
// fixtures. CI runs `cargo test --bin alps-ui` WITHOUT `--features
// server`, so the test profile resolves to this stub. Missing
// `::new()` was the root cause of the 2026-08-26 CI failure.
#[cfg(not(feature = "server"))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct LogLine {
    pub line_no: u64,
    pub text: String,
}

#[cfg(not(feature = "server"))]
impl LogLine {
    // `allow(dead_code)` because clippy runs against the default
    // (no --features server) build profile where the `api::log` real
    // type AND its `new()` don't exist. Tests construct LogLine via
    // this stub, but clippy's default profile doesn't traverse
    // `#[cfg(test)]` mods, so the `new()` call sites are invisible
    // to it. The function is still load-bearing for `cargo test`
    // (the test profile resolves to this stub).
    #[allow(dead_code)]
    pub fn new(line_no: u64, text: String) -> Self {
        Self { line_no, text }
    }
}

// Stub `CommitDiff` for the default (`web`-only) build, mirroring the
// `LogLine` stub above. Same pitfall — M3b's LogLine stub was missing
// `::new()` (Pitfall 32). For `CommitDiff` we don't have a `::new()`
// caller in tests yet, so we just need the field-shape stub for the
// wasm/native `task_diff` stubs above to type-check.
//
// `allow(dead_code)` because clippy's default build profile doesn't
// see the `task_diff` wasm/native stubs (their `cfg` excludes them),
// and the stub `CommitDiff` looks unused. The real `CommitDiff` lives
// in `api::diff` under `feature = "server"`.
#[cfg(not(feature = "server"))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[allow(dead_code)]
pub struct CommitDiff {
    pub sha: String,
    pub author: String,
    pub timestamp: String,
    pub message: String,
    pub patch: String,
}

// Native non-server stub — exists so default builds (no `server` feature,
// no `wasm32` target) can compile. The default build was originally
// wasm-only, but the CI matrix also tests `cargo build --bin alps-ui`
// without features, which is a native target. This stub gives the
// Dashboard a deterministic error message instead of a compile error.
#[cfg(all(not(target_arch = "wasm32"), not(feature = "server")))]
pub async fn tasks_list(_workdir: String) -> Result<crate::domain::TaskList, ServerFnError> {
    Err(ServerFnError(
        "tasks_list requires `--features server` or a wasm build (default `web` feature)".to_string(),
    ))
}

#[cfg(all(not(target_arch = "wasm32"), not(feature = "server")))]
pub async fn task_run(
    _workdir: String,
    _deliverable_path: String,
    _prompt: String,
) -> Result<String, ServerFnError> {
    Err(ServerFnError(
        "task_run requires `--features server` or a wasm build (default `web` feature)".to_string(),
    ))
}

// Native non-server stub for `task_get` — same pattern as the other two.
// Exists so default builds (no `server` feature, no `wasm32` target) can
// compile. Used by the TaskDetail page's non-server-gated placeholder.
#[cfg(all(not(target_arch = "wasm32"), not(feature = "server")))]
pub async fn task_get(
    _workdir: String,
    _task_id: String,
) -> Result<Option<crate::domain::TaskDetail>, ServerFnError> {
    Err(ServerFnError(
        "task_get requires `--features server` or a wasm build (default `web` feature)".to_string(),
    ))
}

// Native non-server stubs for the M3b log-tail endpoints. Same
// pattern as the other fns: default builds (no `server` feature, no
// `wasm32` target) need a compileable stub so the page module compiles.
#[cfg(all(not(target_arch = "wasm32"), not(feature = "server")))]
pub async fn task_log_tail_telemetry(
    _workdir: String,
    _since_line_no: u64,
) -> Result<Vec<LogLine>, ServerFnError> {
    Err(ServerFnError(
        "task_log_tail_telemetry requires `--features server` or a wasm build (default `web` feature)".to_string(),
    ))
}

#[cfg(all(not(target_arch = "wasm32"), not(feature = "server")))]
pub async fn task_log_tail_ralph(
    _workdir: String,
    _task_id: String,
    _since_line_no: u64,
) -> Result<Vec<LogLine>, ServerFnError> {
    Err(ServerFnError(
        "task_log_tail_ralph requires `--features server` or a wasm build (default `web` feature)".to_string(),
    ))
}

// Native non-server stub for `task_cancel` (M3c). Same pattern as
// the log-tail stubs above. `allow(dead_code)` for the default build
// profile where this stub is unreachable.
#[cfg(all(not(target_arch = "wasm32"), not(feature = "server")))]
#[allow(dead_code)]
pub async fn task_cancel(
    _workdir: String,
    _task_id: String,
) -> Result<(), ServerFnError> {
    Err(ServerFnError(
        "task_cancel requires `--features server` or a wasm build (default `web` feature)".to_string(),
    ))
}

// Native non-server stub for `task_diff` (M3c). Uses the local
// `CommitDiff` stub above (also `cfg(not(feature = "server"))`) to
// type-check in default builds. `allow(dead_code)` for default build
// where this stub is unreachable.
#[cfg(all(not(target_arch = "wasm32"), not(feature = "server")))]
#[allow(dead_code)]
pub async fn task_diff(
    _workdir: String,
    _task_id: String,
) -> Result<Vec<CommitDiff>, ServerFnError> {
    Err(ServerFnError(
        "task_diff requires `--features server` or a wasm build (default `web` feature)".to_string(),
    ))
}

// Native non-server stub for `get_workdir` (M4-proper).
#[cfg(all(not(target_arch = "wasm32"), not(feature = "server")))]
#[allow(dead_code)]
pub async fn get_workdir() -> Result<String, ServerFnError> {
    Err(ServerFnError(
        "get_workdir requires `--features server` or a wasm build (default `web` feature)".to_string(),
    ))
}

// Native non-server stub for `set_workdir` (M4-proper).
#[cfg(all(not(target_arch = "wasm32"), not(feature = "server")))]
#[allow(dead_code)]
pub async fn set_workdir(_path: String) -> Result<(), ServerFnError> {
    Err(ServerFnError(
        "set_workdir requires `--features server` or a wasm build (default `web` feature)".to_string(),
    ))
}