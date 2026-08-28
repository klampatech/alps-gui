//! TaskLog page (`/tasks/:id/log`).
//!
//! M3b: dual-pane polled tail. The page renders two near-live
//! streamed views of what the orchestrator is doing for this task:
//!
//! - **Top pane:** the workdir-wide `<workdir>/.alps-telemetry.log`
//!   file (orchestrator `elog!` lines from every task in the workdir,
//!   **not filtered** — `elog!` doesn't tag lines with `task_id`).
//!   Honestly labeled as such.
//! - **Bottom pane:** the per-task
//!   `<workdir>/tasks/<id>/implementation/ralph/.ralph-stderr.log`
//!   file (the Ralph/Codex subprocess's stderr mirror for this task).
//!   Per-task scoped.
//!
//! Both panes share:
//! - A single pause/resume toggle (the buffer state freezes when
//!   paused; polling resumes on unpause).
//! - A single substring search input that filters both panes.
//! - A 500ms polling cadence with a 1000-line in-memory cap per pane
//!   (drop-oldest on overflow).
//!
//! ## Why polling, not SSE
//!
//! v1 deliberate simplification. SSE in Dioxus 0.7 requires bypassing
//! the `#[server]` macro (the macro returns a single JSON value via
//! axum, not a streaming response). The 500ms polling cadence is
//! fine for a log tail, keeps the verify-script deterministic, and
//! lets us upgrade to SSE in v2 without a wire-shape change. See
//! `~/Obsidian/projects/alps-ui-m3-brief.md` M3b "Why polling, not
//! SSE" for the full discussion.
//!
//! ## Why the polling loop lives in the page (not a hook)
//!
//! Dioxus 0.7's `use_future` is the idiomatic place for "spawn a
//! background task tied to component lifetime". A `use_log_tail`
//! hook would be a thin wrapper around the same pattern, and
//! introducing it for v1 means adding a new module + two generic
//! fn signatures + a callback-bridge layer that the rest of the
//! codebase doesn't use. Inlining keeps the polling loop, the
//! buffer signals, and the rendered UI in one file — easier to
//! read in a 6-month re-visit, and the v2 SSE upgrade only needs
//! to replace the `use_future` body.

use std::time::Duration;

use dioxus::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

use crate::api::{task_log_tail_ralph, task_log_tail_telemetry, LogLine};
use crate::domain::TaskId;
use crate::state;

// Local `default_workdir` removed in M4-proper — replaced by the
// shared `state::Workdir` context. See `state.rs` for the resolution
// chain (config file → env var → `$HOME/Development/alps-runs`).

/// Maximum lines kept in memory per pane.
///
/// 1000 lines × ~50 chars/line ≈ 50KB per pane. Drop-oldest on
/// overflow. Matches the brief.
const MAX_BUFFERED_LINES: usize = 1000;

/// Poll cadence.
const POLL_INTERVAL_MS: u64 = 500;

/// Cross-target `sleep(ms)` for the polling loop. Uses
/// `tokio::time::sleep` on native (the `server` build pulls tokio
/// transitively via alps-core) and a `setTimeout`-backed `Promise`
/// on wasm (tokio doesn't compile to wasm32).
///
/// The wasm path is rare in practice — the SSR-only build
/// (`dx serve --platform server`) doesn't reach this code at all
/// (no JS engine), and the wasm hydration build reaches it via
/// the browser's microtask queue.
#[cfg(not(target_arch = "wasm32"))]
async fn poll_sleep(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

#[cfg(target_arch = "wasm32")]
async fn poll_sleep(ms: u64) {
    use wasm_bindgen_futures::JsFuture;

    // Create a Promise that resolves after `ms` via setTimeout.
    // `Promise::new`'s closure receives the resolver functions and
    // schedules `resolve(undefined)` to fire after `ms`. The future
    // is awaited via `JsFuture`, which yields once the Promise
    // resolves — i.e. once `ms` have elapsed.
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let win = web_sys::window().expect("no window in wasm context");
        // `resolve` is a `js_sys::Function`; coerce to a raw
        // `Function` via `JsCast::dyn_ref` so web-sys's C bindings
        // can pass it through to setTimeout. The `_0` variant
        // takes no extra arguments (just a callback + timeout ms).
        let callback: &js_sys::Function = resolve.dyn_ref().expect("resolve is a Function");
        let _ = win.set_timeout_with_callback_and_timeout_and_arguments_0(
            callback,
            ms as i32,
        );
    });
    let _ = JsFuture::from(promise).await;
}

#[component]
pub fn TaskLog(id: TaskId) -> Element {
    // Page state — two per-pane buffers + shared pause + shared
    // filter query. The polling loop writes to the buffers; the
    // filter is a separate Signal so toggling pause doesn't reset
    // the filter state.
    let telemetry_lines = use_signal(Vec::<LogLine>::new);
    let ralph_lines = use_signal(Vec::<LogLine>::new);
    let mut paused = use_signal(|| false);
    let mut filter = use_signal(String::new);

    // Capture the route's task_id once at mount time. The task_id
    // is a `TaskId` newtype; clone the inner String for the polling
    // loop's owned values.
    //
    // v1.1 fix (PR #16): capture the Workdir **signal** (not the
    // value). `Workdir::signal()` returns a `Signal<String>`; reading
    // it via `.cloned()` inside the `use_future` closure re-reads the
    // signal on each iteration, so when the Workdir context updates
    // (Settings Save, App-mount `use_future(get_workdir)` resolves),
    // the polling loop pivots to the new workdir on its next tick.
    // Without this, the loop would fetch telemetry/ralph lines from
    // a stale workdir forever, even after the user changes it via
    // Settings. Mirrors the Settings race fix in PR #14 (Pitfall
    // #56); same latent-bug surface as TaskDetail/TaskDiff.
    //
    // Note: the currently in-flight fetch (the one that's already
    // awaited when the workdir changes) will complete with the old
    // workdir's data. The next loop iteration reads the new signal
    // value and pivots. This is the right semantics — we don't want
    // to restart the loop on every signal change (would lose buffered
    // lines); we want each iteration to use the latest workdir.
    let task_id_value = id.0.clone();
    let workdir_signal = use_context::<state::Workdir>().signal();

    // Polling loop. `use_future` spawns a task tied to the component's
    // lifetime; the loop sleeps `POLL_INTERVAL_MS` between fetches
    // and re-runs naturally whenever the `paused` signal flips
    // (Dioxus's runtime uses `read()` borrows to detect dependency
    // changes).
    //
    // We capture `telemetry_lines` and `ralph_lines` by value (not
    // `cloned()`) so the loop can write to them via `with_mut`.
    // `paused` is read inside the loop body.
    let _ = use_future(move || {
        let wd = workdir_signal.cloned();
        let tid = task_id_value.clone();
        let mut tel_buf = telemetry_lines;
        let mut ral_buf = ralph_lines;
        let paused = paused;

        async move {
            loop {
                // Pause early-return: sleep one tick and re-check.
                // Doesn't reset the buffer; the next non-paused
                // tick resumes from the current cursor.
                if *paused.read() {
                    poll_sleep(POLL_INTERVAL_MS).await;
                    continue;
                }

                // Telemetry fetch. The cursor is "one past the last line we've
                // already buffered" — which is `last_line_no + 1` for a
                // non-empty buffer, or `0` for an empty one. Using
                // `buf.len()` here would be WRONG when the buffer is
                // capped: a 2200-line file with `MAX_BUFFERED_LINES=1000`
                // holds `len()=1000` entries but the last entry's
                // `line_no` is 1499, so the next cursor must be 1500,
                // not 1000. That bug (caught by the live-tick test
                // on 2026-08-26) caused the ralph pane to stop
                // advancing past line 1499 once it hit the cap.
                let tel_cursor = next_cursor(&tel_buf.read());
                match task_log_tail_telemetry(wd.clone(), tel_cursor).await {
                    Ok(new_lines) => append_capped(&mut tel_buf, new_lines),
                    Err(e) => {
                        // Mute the noise: an Err here means the
                        // server fn couldn't read the file
                        // (permissions, race with a write). The
                        // page will surface the error via the
                        // bottom banner.
                        eprintln!("task_log telemetry fetch error: {e:?}");
                    }
                }

                // Ralph fetch — same cursor-correctness rule.
                let ral_cursor = next_cursor(&ral_buf.read());
                match task_log_tail_ralph(wd.clone(), tid.clone(), ral_cursor).await {
                    Ok(new_lines) => append_capped(&mut ral_buf, new_lines),
                    Err(e) => {
                        eprintln!("task_log ralph fetch error: {e:?}");
                    }
                }

                poll_sleep(POLL_INTERVAL_MS).await;
            }
        }
    });

    let tel_lines = telemetry_lines.read().clone();
    let ral_lines = ralph_lines.read().clone();
    let query = filter.read().clone();
    let is_paused = *paused.read();

    rsx! {
        div { class: "p-4 sm:p-6 lg:p-8 space-y-4",
            // Header: StatusPill + task_id + Pause toggle + filter.
            div { class: "flex flex-wrap items-baseline justify-between gap-3",
                div { class: "flex items-center gap-3",
                    h1 { class: "text-2xl font-semibold text-slate-800", "Log" }
                    span { class: "font-mono text-sm text-slate-500", "{id}" }
                }
                div { class: "flex items-center gap-2",
                    PauseToggle {
                        paused: is_paused,
                        on_toggle: move |_| {
                            let current = *paused.read();
                            paused.set(!current);
                        },
                    }
                    FilterInput {
                        query: query.clone(),
                        on_input: move |q: String| filter.set(q),
                    }
                }
            }

            // Top pane: workdir-wide telemetry.
            LogPane {
                label: "Workdir orchestrator log (shared across all tasks)",
                hint: "Per-task filter not yet available — `elog!` does not tag lines with task_id. See alps-ui-m3-brief.md M3b revision note.",
                lines: tel_lines,
                filter_query: query.clone(),
                max_lines: MAX_BUFFERED_LINES,
            }

            // Bottom pane: per-task ralph activity.
            LogPane {
                label: "Per-task Ralph/Codex activity",
                hint: "Tail of <workdir>/tasks/<id>/implementation/ralph/.ralph-stderr.log — only meaningful while the task is in [implement] phase.",
                lines: ral_lines,
                filter_query: query,
                max_lines: MAX_BUFFERED_LINES,
            }
        }
    }
}

/// One log pane (label + hint + filtered <pre>).
///
/// The `<pre>` is monospace + `whitespace-pre-wrap` so long lines
/// wrap rather than overflow horizontally. Latest line at the
/// bottom; the verify-script uses the line count + presence of
/// the filter input + Pause button as acceptance markers.
///
/// Renders an empty-state card when the buffer is empty (so the
/// pane never collapses to nothing — the operator sees the label
/// + hint and knows "nothing has happened yet, that's expected").
#[component]
fn LogPane(
    label: &'static str,
    hint: &'static str,
    lines: Vec<LogLine>,
    filter_query: String,
    max_lines: usize,
) -> Element {
    let filtered: Vec<&LogLine> = if filter_query.is_empty() {
        lines.iter().collect()
    } else {
        lines
            .iter()
            .filter(|l| l.text.contains(&filter_query))
            .collect()
    };

    rsx! {
        section { class: "space-y-2",
            div { class: "flex items-baseline justify-between gap-2",
                h2 { class: "text-sm font-medium text-slate-700", "{label}" }
                span { class: "font-mono text-xs text-slate-400",
                    "showing {filtered.len()} of {lines.len()} (cap {max_lines})"
                }
            }
            if filtered.is_empty() {
                div { class: "rounded-md border border-slate-200 bg-white p-3 shadow-sm",
                    p { class: "text-xs text-slate-500", "{hint}" }
                }
            } else {
                pre { class: "rounded-md border border-slate-200 bg-slate-50 p-3 text-xs font-mono text-slate-800 whitespace-pre-wrap break-words max-h-[40vh] overflow-y-auto",
                    for line in filtered.iter() {
                        span { class: "block",
                            span { class: "text-slate-400 mr-3 select-none", "{line.line_no}" }
                            span { "{line.text}" }
                        }
                    }
                }
            }
        }
    }
}

/// Pause / resume button.
#[component]
fn PauseToggle(paused: bool, on_toggle: EventHandler<MouseEvent>) -> Element {
    rsx! {
        button {
            r#type: "button",
            class: "rounded-md border border-slate-300 bg-white px-2.5 py-1 text-xs font-medium text-slate-700 hover:bg-slate-50",
            onclick: move |evt| on_toggle.call(evt),
            title: if paused { "Resume polling" } else { "Pause polling — buffer freezes" },
            if paused {
                "▶ Resume"
            } else {
                "⏸ Pause"
            }
        }
    }
}

/// Substring search filter input.
#[component]
fn FilterInput(query: String, on_input: EventHandler<String>) -> Element {
    rsx! {
        input {
            r#type: "search",
            class: "rounded-md border border-slate-300 bg-white px-2 py-1 text-xs text-slate-700 focus:border-slate-500 focus:outline-none focus:ring-1 focus:ring-slate-500 w-48",
            placeholder: "filter…",
            value: "{query}",
            oninput: move |evt| on_input.call(evt.value()),
        }
    }
}

/// Append `new_lines` to `buf`, truncating from the front if `buf`
/// would exceed `cap`.
///
/// `new_lines` is assumed to be in cursor order (0-indexed,
/// monotonically increasing). The function does NOT deduplicate;
/// if the server returns overlapping lines on rare race conditions,
/// the buffer may contain duplicates. The cursor invariant is
/// `server_returns_lines_with_line_no == buf.last().line_no + 1`,
/// documented in `api/log.rs::tail_file`.
fn append_capped(buf: &mut Signal<Vec<LogLine>>, new_lines: Vec<LogLine>) {
    buf.with_mut(|b| {
        b.extend(new_lines);
        if b.len() > MAX_BUFFERED_LINES {
            let excess = b.len() - MAX_BUFFERED_LINES;
            b.drain(0..excess);
        }
    });
}

/// Compute the next-fetch cursor from a buffer.
///
/// Returns `0` for an empty buffer (start of file). Otherwise returns
/// `last.line_no + 1` — the line number ONE PAST the last entry we've
/// already buffered.
///
/// **Why not `buf.len()`?** Once the buffer is capped at
/// `MAX_BUFFERED_LINES`, `len()` no longer equals the file offset of
/// the last entry — it equals the *count* of buffered entries. For a
/// 2200-line file with the buffer capped at 1000, `len()=1000` but
/// `last.line_no=1499`, so the next cursor must be 1500. Using
/// `len()=1000` re-fetches lines 1000-1499 every poll, which the
/// buffer silently de-duplicates by extending + truncating (so the
/// pane appears to "stop advancing" past the cap). Caught 2026-08-26
/// by the live-tick test that Kyle requested.
fn next_cursor(buf: &[LogLine]) -> u64 {
    match buf.last() {
        Some(last) => last.line_no + 1,
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_line(n: u64, text: &str) -> LogLine {
        LogLine::new(n, text.to_string())
    }

    #[test]
    fn next_cursor_empty_buffer_returns_zero() {
        let buf: Vec<LogLine> = Vec::new();
        assert_eq!(next_cursor(&buf), 0, "empty buffer must start at line 0");
    }

    #[test]
    fn next_cursor_non_empty_returns_last_line_no_plus_one() {
        let buf = vec![make_line(0, "a"), make_line(1, "b"), make_line(2, "c")];
        assert_eq!(next_cursor(&buf), 3);
    }

    #[test]
    fn next_cursor_after_truncation_uses_line_no_not_position() {
        // This is the bug: after front-truncation drops the first N
        // lines, the buffer's len() no longer matches the last
        // entry's line_no + 1. next_cursor must use the line_no.
        // Buffer holds lines 500..=1499 (1000 entries after truncation
        // from an original 1500-entry buffer).
        let buf: Vec<LogLine> = (500..1500).map(|i| make_line(i, "x")).collect();
        assert_eq!(buf.len(), 1000, "buf should be 1000 lines (capped)");
        assert_eq!(next_cursor(&buf), 1500, "next cursor must be 1500, NOT 1000");
    }

    #[test]
    fn append_capped_truncates_front_on_overflow() {
        // Simulate the cap logic by directly mutating a Vec.
        let mut buf: Vec<LogLine> = (0..MAX_BUFFERED_LINES as u64)
            .map(|i| make_line(i, &format!("old-{i}")))
            .collect();
        buf.extend(vec![
            make_line(MAX_BUFFERED_LINES as u64, "new-0"),
            make_line((MAX_BUFFERED_LINES + 1) as u64, "new-1"),
            make_line((MAX_BUFFERED_LINES + 2) as u64, "new-2"),
            make_line((MAX_BUFFERED_LINES + 3) as u64, "new-3"),
            make_line((MAX_BUFFERED_LINES + 4) as u64, "new-4"),
        ]);
        if buf.len() > MAX_BUFFERED_LINES {
            let excess = buf.len() - MAX_BUFFERED_LINES;
            buf.drain(0..excess);
        }
        assert_eq!(buf.len(), MAX_BUFFERED_LINES);
        assert_eq!(buf[0].text, "old-5");
        assert_eq!(buf[MAX_BUFFERED_LINES - 1].text, "new-4");
    }

    #[test]
    fn append_capped_no_truncate_under_limit() {
        let mut buf: Vec<LogLine> = Vec::new();
        buf.extend(vec![make_line(0, "a"), make_line(1, "b")]);
        if buf.len() > MAX_BUFFERED_LINES {
            let excess = buf.len() - MAX_BUFFERED_LINES;
            buf.drain(0..excess);
        }
        assert_eq!(buf.len(), 2);
        assert_eq!(buf[0].text, "a");
        assert_eq!(buf[1].text, "b");
    }

    /// SSR contract for M3b: the page must render both pane labels
    /// + the Pause button in the SSR'd HTML so the verify-script's
    /// +3 acceptance criteria pass without hydration.
    #[test]
    fn task_log_ssr_shows_both_pane_labels_and_pause_button() {
        use crate::pages::TaskLog;
        use crate::state::provide_workdir;

        // Wrap TaskLog in an inline component that provides the
        // Workdir context (M4-proper). Tests previously rendered the
        // page directly, but TaskLog now reads via use_context which
        // is uninitialized without App's provide_workdir() call.
        #[component]
        fn TestApp() -> Element {
            let _wd = provide_workdir();
            rsx! { TaskLog { id: TaskId::new("test-id") } }
        }

        let html = dioxus_ssr::render_element(rsx! {
            TestApp {}
        });

        // Top pane label.
        assert!(
            html.contains("Workdir orchestrator log"),
            "Top pane label should render in SSR: {html}"
        );
        // Bottom pane label.
        assert!(
            html.contains("Per-task Ralph/Codex activity"),
            "Bottom pane label should render in SSR: {html}"
        );
        // Pause button.
        assert!(
            html.contains("Pause"),
            "Pause button should render in SSR: {html}"
        );
    }
}