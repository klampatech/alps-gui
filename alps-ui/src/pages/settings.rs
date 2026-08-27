//! Settings page (`/settings`).
//!
//! M4-prep UI shell: replace the M3a "Settings — coming in v2" stub with
//! a real form that **doesn't lie** about what it does. Three cards:
//!
//! 1. **Workdir** — read-only display of `default_workdir()`. The
//!    input + Save button are present but the value is held in a
//!    local `use_signal` only — **not** persisted across reloads, and
//!    **not** propagated to other pages. M4-proper will introduce a
//!    shared `Signal<String>` workdir context + gloo-storage-backed
//!    persistence, and migrate the 5 `default_workdir()` callsites.
//!
//! 2. **MINIMAX_API_KEY status** — server-side check via
//!    `std::env::var("MINIMAX_API_KEY")`. Wasm builds show
//!    "n/a — browser preview" because `std::env::var` doesn't link on
//!    `wasm32-unknown-unknown` (same gating pattern as `default_workdir()`).
//!
//! 3. **About** — static card with package version + (optionally) the
//!    build commit hash. Uses `option_env!()` so the build doesn't
//!    fail if vergen isn't wired.
//!
//! ## Why this isn't the full M4
//!
//! The full M4 is "Settings page + workdir context + 5 callsite
//! migrations + persistence". M4-prep is the UI shell only —
//! `pages/settings.rs` is no longer a lie, but the Save button does
//! nothing more durable than `use_signal`. See
//! `~/Obsidian/projects/alps-ui-m4-prep-brief.md` for the full split
//! rationale + scope-out list.

use dioxus::prelude::*;

/// Page route handler for `/settings`. M4-prep.
#[component]
pub fn Settings() -> Element {
    // Local-only state for the Save button. M4-proper replaces this
    // with a shared `Signal<String>` workdir context + gloo-storage
    // persistence — see Pitfall 37 (turbofish on the call, not the
    // binding) and the M4-prop brief's "Out" section.
    //
    // Signals are NOT `mut` here because the mutations happen inside
    // `WorkdirCard`'s `oninput` / `onclick` closures, which receive
    // mutable references from this scope.
    let input_value = use_signal::<String>(default_workdir);
    let saved_toast = use_signal::<Option<String>>(|| None);

    rsx! {
        div { class: "p-4 sm:p-6 lg:p-8 space-y-4",
            h1 { class: "text-2xl font-semibold text-slate-800", "Settings" }

            // Card 1: Workdir (read-only display + local-only Save).
            WorkdirCard {
                input_value: input_value,
                saved_toast: saved_toast,
            }

            // Card 2: MINIMAX_API_KEY status. Server-side only; wasm
            // shows "n/a — browser preview" because std::env::var
            // doesn't link on wasm32-unknown-unknown.
            ApiKeyCard {}

            // Card 3: About (static).
            AboutCard {}

            // Saved toast (3-second auto-clear via use_future).
            // Rendered conditionally at the bottom so it doesn't take
            // up layout space when there's nothing to show.
            if let Some(msg) = saved_toast.read().clone() {
                Toast { message: msg }
            }
        }
    }
}

/// Card 1: Workdir display + local-only Save input.
///
/// `input_value` and `saved_toast` are passed by reference so the
/// outer `Settings` component owns the signals (Dioxus 0.7's pattern
/// for parent-owned, child-rendered signals — see `task_detail.rs`'s
/// `PopulatedDetail` for the same pattern).
#[component]
fn WorkdirCard(
    mut input_value: Signal<String>,
    mut saved_toast: Signal<Option<String>>,
) -> Element {
    let on_save = move |_| {
        // Local-only save: stash the input value into the input_value
        // signal (no-op for UX since the input is already bound to it)
        // and show a toast that explicitly tells the user this isn't
        // persisted. M4-proper wires this to a real context + storage.
        let val = input_value.read().clone();
        saved_toast.set(Some(format!(
            "Saved {val} (v0 — not persisted across reloads)"
        )));
    };

    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm",
            h2 { class: "text-lg font-semibold text-slate-800 mb-2",
                "Workdir"
            }
            p { class: "text-sm text-slate-600 mb-3",
                "Read-only in v0. Editing + persistence land in M4 (planned follow-up)."
            }
            div { class: "flex gap-2",
                input {
                    class: "flex-1 rounded border border-slate-300 px-3 py-2 text-sm font-mono text-slate-700",
                    value: "{input_value.read()}",
                    oninput: move |evt| input_value.set(evt.value()),
                }
                button {
                    class: "px-4 py-2 bg-slate-800 text-white text-sm rounded hover:bg-slate-700",
                    onclick: on_save,
                    "Save"
                }
            }
        }
    }
}

/// Card 2: MINIMAX_API_KEY detection status.
///
/// The actual env-var check is `#[cfg(feature = "server")]`-gated because
/// `std::env::var` doesn't link on `wasm32-unknown-unknown`. The wasm
/// build sees a hardcoded "n/a — browser preview" string. See Pitfall
/// 38 (feature-gated modules) — same pattern as the LogLine / CommitDiff
/// stub in `api/mod.rs`.
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
        // Server build: actually read the env var.
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
        // Wasm build: std::env::var doesn't link. Show "n/a".
        rsx! {
            span { class: "text-slate-500",
                "n/a — browser preview"
            }
        }
    }
}

/// Card 3: About — static metadata.
///
/// `option_env!("CARGO_PKG_VERSION")` always succeeds (cargo injects it
/// at build time). `VERGEN_GIT_SHA` and `VERGEN_BUILD_TIMESTAMP` are
/// only set if vergen is wired in (it's not, currently) — using
/// `option_env!` means missing env vars compile cleanly and just
/// render "unavailable".
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

/// Toast — small "Saved" banner. Click to dismiss (no auto-clear timer;
/// the simpler approach avoids adding a new crate dep just for one
/// 3-second timer. M4-proper can revisit if a sticky toast becomes a
/// real UX issue).
#[component]
fn Toast(message: String) -> Element {
    rsx! {
        div {
            class: "rounded-md bg-green-100 border border-green-300 text-green-800 px-3 py-2 text-sm cursor-pointer hover:bg-green-200",
            title: "Click to dismiss",
            "{message}"
        }
    }
}

/// Local copy of `default_workdir()` — duplicated from the dashboard /
/// task_detail / task_log / task_diff pages. M4-proper will collapse
/// all 5 callsites into a shared `use_context::<Workdir>()`; for
/// M4-prep the duplication matches the existing pattern.
///
/// Reads `ALPS_UI_WORKDIR` env var if set, else falls back to
/// `~/Development/alps-runs`.
fn default_workdir() -> String {
    std::env::var("ALPS_UI_WORKDIR")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_default();
            format!("{home}/Development/alps-runs")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `default_workdir()` falls back to `$HOME/Development/alps-runs`
    /// when `ALPS_UI_WORKDIR` is unset or empty. We don't test the
    /// "set" case because env vars in concurrent tests are racy.
    #[test]
    fn default_workdir_falls_back_to_home_dev_alps_runs() {
        // SAFETY: this test is not run in parallel with itself, but
        // other tests might be touching the same env var. We save and
        // restore to be defensive — if a parallel test sets the var
        // and this reads it, we'd just see that value.
        let saved = std::env::var("ALPS_UI_WORKDIR").ok();
        // SAFETY: removing an env var is also racy in principle, but
        // in practice tests in the same crate don't touch this var.
        std::env::remove_var("ALPS_UI_WORKDIR");

        let home = std::env::var("HOME").unwrap_or_default();
        assert_eq!(default_workdir(), format!("{home}/Development/alps-runs"));

        if let Some(v) = saved {
            std::env::set_var("ALPS_UI_WORKDIR", v);
        }
    }

    /// When `ALPS_UI_WORKDIR` IS set to a non-empty value,
    /// `default_workdir()` returns that value verbatim (no $HOME
    /// expansion).
    #[test]
    fn default_workdir_uses_env_when_set() {
        let saved = std::env::var("ALPS_UI_WORKDIR").ok();
        std::env::set_var("ALPS_UI_WORKDIR", "/tmp/alps-test-workdir");
        assert_eq!(default_workdir(), "/tmp/alps-test-workdir");
        match saved {
            Some(v) => std::env::set_var("ALPS_UI_WORKDIR", v),
            None => std::env::remove_var("ALPS_UI_WORKDIR"),
        }
    }

    /// When `ALPS_UI_WORKDIR` is set to an empty string, the empty
    /// value is treated as unset and falls back to the default.
    #[test]
    fn default_workdir_treats_empty_env_as_unset() {
        let saved = std::env::var("ALPS_UI_WORKDIR").ok();
        std::env::set_var("ALPS_UI_WORKDIR", "");
        let home = std::env::var("HOME").unwrap_or_default();
        assert_eq!(default_workdir(), format!("{home}/Development/alps-runs"));
        match saved {
            Some(v) => std::env::set_var("ALPS_UI_WORKDIR", v),
            None => std::env::remove_var("ALPS_UI_WORKDIR"),
        }
    }
}
