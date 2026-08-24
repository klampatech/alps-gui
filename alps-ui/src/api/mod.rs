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
pub async fn tasks_list(workdir: String) -> Result<crate::domain::TaskList, ServerFnError> {
    use serde::Serialize;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::JsValue;
    use wasm_bindgen_futures::JsFuture;

    #[derive(Serialize)]
    struct Args {
        workdir: String,
    }

    let args = Args { workdir };
    let body = serde_json::to_string(&args)
        .map_err(|e| ServerFnError(format!("serialize args: {e}")))?;

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_body(&JsValue::from_str(&body));

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let hash_input = format!("{manifest_dir}:alps_ui::api::tasks");
    let hash = xxhash_rust::const_xxh64::xxh64(hash_input.as_bytes(), 0);
    let url = format!("/api/tasks_list{hash}");
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
        .map_err(|e| ServerFnError(format!("parse TaskList: {e}; body={text}")))
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