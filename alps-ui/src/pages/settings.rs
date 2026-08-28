//! Settings page (`/settings`).
//!
//! M4-prep shipped the UI shell (3 cards + local-only Save). M4-proper
//! wires the Save button to the real `set_workdir` server fn, which
//! persists the workdir path to `$HOME/.alps-ui-config.json`. The
//! Workdir context is then updated so every other page reads the new
//! value via `use_context::<state::Workdir>()`.
//!
//! ## Layout
//!
//! 1. **Workdir** — input + Save button. Save handler calls
//!    `set_workdir` server fn (which atomically writes
//!    `$HOME/.alps-ui-config.json`), then updates the local
//!    `Workdir` context. On success: green toast "Saved <path>".
//!    On error: red toast with the error message.
//! 2. **MINIMAX_API_KEY status** — server-side `std::env::var` check
//!    (wasm shows "n/a — browser preview" since `std::env::var`
//!    doesn't link on `wasm32-unknown-unknown`).
//! 3. **About** — package version + optional build commit/time via
//!    `option_env!` so the build doesn't fail if vergen isn't wired.
//!
//! ## Why server-side persistence only (no gloo-storage)
//!
//! Decision recorded 2026-08-26 per Kyle: "server makes sense then".
//! The workdir is a single-host filesystem concept (the alps-runs
//! directory). Browser-side persistence would create a "what if the
//! browser says X and the server says Y" reconciliation question
//! that has no upside for the alps-runs use case. Future multi-workdir
//! picker needs server-side as the source of truth anyway.

use dioxus::prelude::*;

use crate::api::set_workdir;
use crate::state;

/// Page route handler for `/settings`. M4-proper: the Save button
/// wires through `set_workdir` server fn + `Workdir` context.
///
/// ## v1.1 fix — Settings initial-load race
///
/// Before v1.1, this page called `workdir_ctx.get()` once at mount and
/// bound the result to a local `use_signal`. That meant the input
/// showed the wasm-side fallback workdir (`~/.alps-runs`) on fresh
/// loads — because the App-mount `use_future(get_workdir)` resolved
/// AFTER this component mounted and didn't re-trigger the local
/// signal. The input briefly displayed the wrong value before the
/// user clicked Save (or reloaded). Pitfall #56 in
/// `references/dioxus-0.7-m5-pitfalls.md`.
///
/// Fix: derive the input value reactively from the Workdir context
/// signal. Use an `Option<String>` "draft" signal that captures the
/// user's unsaved edits — the input reads `draft.unwrap_or(workdir)`,
/// so the value tracks both the persisted state AND the in-flight edit.
/// On Save, clear the draft (now the Workdir context is the source of
/// truth). On server error, keep the draft so the user can retry.
#[component]
pub fn Settings() -> Element {
    // Capture the Workdir context once. The Save closure is `move`
    // and needs to call `set` after the server fn returns — so we
    // capture by value (it's `Copy`).
    let workdir_ctx = use_context::<state::Workdir>();

    // `workdir_signal` is the reactive read — Dioxus tracks the read
    // and re-renders this component when the Workdir signal updates
    // (e.g. after App-mount `use_future(get_workdir)` resolves, or
    // after this page's Save handler calls `workdir_ctx.set(...)`).
    let workdir_signal = workdir_ctx.signal();

    // Draft state — captures the user's in-flight edit. `None` means
    // "show the current persisted workdir". `Some(s)` means "show the
    // user's edit, even if it differs from the persisted value".
    // Cleared on successful Save (the Workdir signal updates, which
    // already reflects the new value) and on user-initiated reload
    // (back to None).
    let mut draft = use_signal::<Option<String>>(|| None);

    let saved_toast = use_signal::<Option<String>>(|| None);
    let mut saving = use_signal(|| false);

    let on_save = move |_| {
        // Read the displayed value (= draft if set, else current
        // workdir). On a no-op Save (user didn't edit anything), this
        // is the persisted value, which is safe to re-POST.
        let val = draft.read().clone().unwrap_or_else(|| workdir_signal.cloned());
        saving.set(true);
        // Capture mutable refs for the async closure. The closure
        // must be 'move' to take ownership of the values.
        let mut workdir_ctx = workdir_ctx;
        let mut saving = saving;
        let mut saved_toast = saved_toast;
        let mut draft = draft;
        spawn(async move {
            match set_workdir(val.clone()).await {
                Ok(()) => {
                    // Server-side persistence succeeded — update the
                    // shared context so every page sees the new path.
                    // The input's reactive read will re-derive to the
                    // new persisted value, so we can clear the draft.
                    workdir_ctx.set(val.clone());
                    draft.set(None);
                    saved_toast.set(Some(format!("Saved {val}")));
                }
                Err(e) => {
                    // Keep the draft so the user can retry without
                    // retyping. The persisted workdir signal is
                    // unchanged, but the input keeps showing the edit.
                    saved_toast.set(Some(format!("Save failed: {e:?}")));
                }
            }
            saving.set(false);
        });
    };

    rsx! {
        div { class: "p-4 sm:p-6 lg:p-8 space-y-4",
            h1 { class: "text-2xl font-semibold text-slate-800", "Settings" }

            // Card 1: Workdir (real Save — calls set_workdir server fn).
            // The WorkdirCard is now pure presentational — the parent
            // owns the draft state and reads `workdir_signal` reactively.
            // The `input_value` passed here is computed each render from
            // the current draft OR the current persisted workdir.
            WorkdirCard {
                input_value: draft.read().clone().unwrap_or_else(|| workdir_signal.cloned()),
                on_input: move |new: String| draft.set(Some(new)),
                saving: saving,
                on_save: on_save,
            }

            // Card 2: MINIMAX_API_KEY status. Server-side only.
            ApiKeyCard {}

            // Card 3: About (static).
            AboutCard {}

            // Saved / failed toast. Click to dismiss.
            if let Some(msg) = saved_toast.read().clone() {
                Toast { message: msg }
            }
        }
    }
}

/// Card 1: Workdir input + Save button. Pure presentational — all
/// state lives in the parent `Settings` component.
///
/// `input_value` is the current input string (either the user's draft
/// edit OR the persisted workdir). `on_input` fires on every
/// keystroke, letting the parent update its draft state. `on_save`
/// fires when the user clicks Save (the parent calls the server fn).
#[component]
fn WorkdirCard(
    input_value: String,
    on_input: EventHandler<String>,
    saving: Signal<bool>,
    on_save: EventHandler<MouseEvent>,
) -> Element {
    let is_saving = *saving.read();
    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm",
            h2 { class: "text-lg font-semibold text-slate-800 mb-2",
                "Workdir"
            }
            p { class: "text-sm text-slate-600 mb-3",
                "Edit the workdir and click Save to persist to "
                span { class: "font-mono text-xs", "$HOME/.alps-ui-config.json" }
                ". Server-side only — the change applies across all browser tabs + survives alps-ui server restarts."
            }
            div { class: "flex gap-2",
                input {
                    class: "flex-1 rounded border border-slate-300 px-3 py-2 text-sm font-mono text-slate-700",
                    value: "{input_value}",
                    disabled: "{is_saving}",
                    oninput: move |evt| on_input.call(evt.value()),
                }
                button {
                    class: "px-4 py-2 bg-slate-800 text-white text-sm rounded hover:bg-slate-700 disabled:opacity-50 disabled:cursor-not-allowed",
                    style: "flex-shrink: 0; width: 6rem;",
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
}

/// Card 2: MINIMAX_API_KEY detection status. Same as M4-prep — the
/// actual env-var check is `#[cfg(feature = "server")]`-gated because
/// `std::env::var` doesn't link on `wasm32-unknown-unknown`.
#[component]
fn ApiKeyCard() -> Element {
    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm",
            h2 { class: "text-lg font-semibold text-slate-800 mb-2",
                "MINIMAX_API_KEY"
            }
            p { class: "text-sm text-slate-600 mb-2",
                "Environment variable detection. Value is never displayed."
            }
            div { class: "text-sm font-mono",
                ApiKeyStatus {}
            }
        }
    }
}

/// Inner component for the env-var check. Separated so the
/// `#[cfg(feature = "server")]` gate applies to just the check, not
/// the whole card chrome.
#[component]
fn ApiKeyStatus() -> Element {
    #[cfg(feature = "server")]
    {
        match std::env::var("MINIMAX_API_KEY") {
            Ok(_) => rsx! {
                span { class: "text-green-700",
                    "Detected (value not displayed)"
                }
            },
            Err(_) => rsx! {
                span { class: "text-amber-700",
                    "Not set in environment"
                }
            },
        }
    }
    #[cfg(not(feature = "server"))]
    {
        rsx! {
            span { class: "text-slate-500",
                "n/a — browser preview"
            }
        }
    }
}

/// Card 3: About — static metadata. Uses `option_env!` so the build
/// doesn't fail when vergen isn't wired (option_env returns None →
/// fallback string).
#[component]
fn AboutCard() -> Element {
    let version = env!("CARGO_PKG_VERSION");
    let git_sha = option_env!("VERGEN_GIT_SHA").unwrap_or("(unavailable in this build)");
    let build_ts = option_env!("VERGEN_BUILD_TIMESTAMP").unwrap_or("(unavailable in this build)");

    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm",
            h2 { class: "text-lg font-semibold text-slate-800 mb-2",
                "About"
            }
            dl { class: "grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1 text-sm",
                dt { class: "font-semibold text-slate-700", "Version" }
                dd { class: "font-mono text-slate-600", "{version}" }

                dt { class: "font-semibold text-slate-700", "Build commit" }
                dd { class: "font-mono text-slate-600", "{git_sha}" }

                dt { class: "font-semibold text-slate-700", "Build time" }
                dd { class: "font-mono text-slate-600", "{build_ts}" }
            }
        }
    }
}

/// Toast — small "Saved" / "Save failed" banner. Click to dismiss.
/// (No auto-clear timer — saves a gloo-timers dep for a 3-second fade.)
#[component]
fn Toast(message: String) -> Element {
    let is_error = message.starts_with("Save failed");
    let bg_class = if is_error {
        "rounded-md bg-red-100 border border-red-300 text-red-800 px-3 py-2 text-sm cursor-pointer hover:bg-red-200"
    } else {
        "rounded-md bg-green-100 border border-green-300 text-green-800 px-3 py-2 text-sm cursor-pointer hover:bg-green-200"
    };
    rsx! {
        div {
            class: "{bg_class}",
            title: "Click to dismiss",
            "{message}"
        }
    }
}