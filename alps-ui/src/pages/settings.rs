//! Settings page (`/settings`).
//!
//! Control Rail redesign (deck §06): three cards (Workdir / MINIMAX_API_KEY /
//! About) in a horizontal grid. Status pills match the same signal palette
//! as task state — "Detected" reads as teal (Done), "n/a" reads as gray (Idle).
//!
//! ## Layout (deck §06)
//!
//! ```text
//! <div p-6 lg:p-8>
//!   <h1>Settings</h1>
//!   <3-col grid>
//!     <WorkdirCard>     — input + Save
//!     <ApiKeyCard>      — Server / Wasm status lines with pills
//!     <AboutCard>       — Version / Commit / Built
//!   </3-col grid>
//!   <Toast>              — "Saved <path>" / "Save failed"
//! </div>
//! ```
//!
//! ## v1.1 fix — reactive initial load (PR #14)
//!
//! Carried over from M4-proper. The input value derives from the
//! Workdir context signal so a fresh page load doesn't briefly show
//! the wasm-side fallback.

use dioxus::prelude::*;

use crate::api::set_workdir;
use crate::state;

#[component]
pub fn Settings() -> Element {
    let workdir_ctx = use_context::<state::Workdir>();
    let workdir_signal = workdir_ctx.signal();
    let mut draft = use_signal::<Option<String>>(|| None);
    let saved_toast = use_signal::<Option<String>>(|| None);
    let mut saving = use_signal(|| false);

    let on_save = move |_| {
        let val = draft.read().clone().unwrap_or_else(|| workdir_signal.cloned());
        saving.set(true);
        let mut workdir_ctx = workdir_ctx;
        let mut saving = saving;
        let mut saved_toast = saved_toast;
        let mut draft = draft;
        spawn(async move {
            match set_workdir(val.clone()).await {
                Ok(()) => {
                    workdir_ctx.set(val.clone());
                    draft.set(None);
                    saved_toast.set(Some(format!("Saved {val}")));
                }
                Err(e) => {
                    saved_toast.set(Some(format!("Save failed: {e:?}")));
                }
            }
            saving.set(false);
        });
    };

    rsx! {
        div {
            style: "padding:24px;",
            h1 {
                class: "alps-mono",
                style: "font-size:24px;font-weight:600;margin:0 0 24px 0;color:var(--text);",
                "Settings"
            }
            // 3-col grid per deck §06.
            div {
                style: "display:grid;grid-template-columns:repeat(3,minmax(0,1fr));gap:16px;margin-bottom:20px;",
                WorkdirCard {
                    input_value: draft.read().clone().unwrap_or_else(|| workdir_signal.cloned()),
                    on_input: move |new: String| draft.set(Some(new)),
                    saving: saving,
                    on_save: on_save,
                }
                ApiKeyCard {}
                AboutCard {}
            }
            if let Some(msg) = saved_toast.read().clone() {
                Toast { message: msg }
            }
        }
    }
}

#[component]
fn WorkdirCard(
    input_value: String,
    on_input: EventHandler<String>,
    saving: Signal<bool>,
    on_save: EventHandler<MouseEvent>,
) -> Element {
    let is_saving = *saving.read();
    rsx! {
        div {
            class: "surface",
            style: "padding:18px 20px;",
            h2 {
                class: "alps-mono",
                style: "font-size:13px;font-weight:600;color:var(--text);margin:0 0 8px 0;",
                "Workdir"
            }
            div {
                class: "alps-mono",
                style: "background:var(--ink);border:1px solid var(--hair-soft);border-radius:8px;padding:9px 12px;font-size:11px;color:var(--dim);margin-bottom:12px;display:flex;align-items:center;justify-content:space-between;",
                input {
                    style: "background:transparent;border:none;color:var(--dim);font-family:var(--mono);font-size:11px;width:100%;outline:none;",
                    value: "{input_value}",
                    disabled: "{is_saving}",
                    oninput: move |evt| on_input.call(evt.value()),
                }
            }
            button {
                class: "btn-ghost",
                style: "width:100%;justify-content:center;",
                disabled: "{is_saving}",
                onclick: move |evt| on_save.call(evt),
                if is_saving {
                    "Saving…"
                } else {
                    "Save"
                }
            }
        }
    }
}

#[component]
fn ApiKeyCard() -> Element {
    rsx! {
        div {
            class: "surface",
            style: "padding:18px 20px;",
            h2 {
                class: "alps-mono",
                style: "font-size:13px;font-weight:600;color:var(--text);margin:0 0 12px 0;",
                "MINIMAX_API_KEY"
            }
            ApiKeyStatus {}
        }
    }
}

#[component]
fn ApiKeyStatus() -> Element {
    #[cfg(feature = "server")]
    {
        match std::env::var("MINIMAX_API_KEY") {
            Ok(_) => rsx! {
                div {
                    class: "status-line",
                    span { "Server" }
                    span { class: "pill pill-done",
                        span { class: "dot" }
                        "Detected"
                    }
                }
                div {
                    class: "status-line",
                    span { "Wasm client" }
                    span { class: "pill pill-idle",
                        span { class: "dot" }
                        "n/a — preview"
                    }
                }
            },
            Err(_) => rsx! {
                div {
                    class: "status-line",
                    span { "Server" }
                    span { class: "pill pill-failed",
                        span { class: "dot" }
                        "Not set"
                    }
                }
                div {
                    class: "status-line",
                    span { "Wasm client" }
                    span { class: "pill pill-idle",
                        span { class: "dot" }
                        "n/a — preview"
                    }
                }
            },
        }
    }
    #[cfg(not(feature = "server"))]
    {
        rsx! {
            div {
                class: "status-line",
                span { "Wasm client" }
                span { class: "pill pill-idle",
                    span { class: "dot" }
                    "n/a — preview"
                }
            }
        }
    }
}

#[component]
fn AboutCard() -> Element {
    let version = env!("CARGO_PKG_VERSION");
    let git_sha = option_env!("VERGEN_GIT_SHA").unwrap_or("(unavailable in this build)");
    let build_ts = option_env!("VERGEN_BUILD_TIMESTAMP").unwrap_or("(unavailable in this build)");

    rsx! {
        div {
            class: "surface",
            style: "padding:18px 20px;",
            h2 {
                class: "alps-mono",
                style: "font-size:13px;font-weight:600;color:var(--text);margin:0 0 12px 0;",
                "About"
            }
            div {
                class: "status-line",
                span { "Version" }
                span {
                    class: "alps-mono",
                    style: "color:var(--dim);",
                    "{version}"
                }
            }
            div {
                class: "status-line",
                span { "Commit" }
                span {
                    class: "alps-mono",
                    style: "color:var(--dim);",
                    "{git_sha}"
                }
            }
            div {
                class: "status-line",
                span { "Built" }
                span {
                    class: "alps-mono",
                    style: "color:var(--dim);",
                    "{build_ts}"
                }
            }
        }
    }
}

#[component]
fn Toast(message: String) -> Element {
    let is_error = message.starts_with("Save failed");
    let cls = if is_error { "toast-err" } else { "toast-ok" };
    rsx! {
        div {
            class: "{cls}",
            style: "margin-top:8px;",
            span {
                class: "alps-mono",
                "{message}"
            }
            span { "×" }
        }
    }
}
