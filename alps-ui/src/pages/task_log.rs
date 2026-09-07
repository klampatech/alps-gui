//! TaskLog page (`/tasks/:id/log`).
//!
//! Control Rail redesign (deck §04): dual-pane polled tail, restyled
//! for the dark palette. Each pane has:
//!
//! - Toolbar with the file path (mono, faint) + a Switch primitive
//!   (deco-only toggle; the deck shows it without wiring).
//! - Filter row (search input + line count).
//! - Log lines color-coded by level — `lv-info` (dim), `lv-warn`
//!   (amber), `lv-ok` (teal), `lv-err` (red). The level is derived
//!   from substring matches against the line text (informational only).
//!
//! ## Why polling, not SSE
//!
//! Carried over from M3b. SSE upgrade is deferred to v2 — wire shape
//! doesn't change.
//!
//! ## Why the polling loop lives in the page (not a hook)
//!
//! Same rationale as M3b. The polling loop, buffer signals, and
//! rendered UI live in one file for readability.

use std::time::Duration;

use dioxus::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

use crate::api::{task_log_tail_ralph, task_log_tail_telemetry, LogLine};
use crate::domain::TaskId;
use crate::state;

const MAX_BUFFERED_LINES: usize = 1000;
const POLL_INTERVAL_MS: u64 = 500;

#[cfg(not(target_arch = "wasm32"))]
async fn poll_sleep(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

#[cfg(target_arch = "wasm32")]
async fn poll_sleep(ms: u64) {
    use wasm_bindgen_futures::JsFuture;
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let win = web_sys::window().expect("no window in wasm context");
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
    let telemetry_lines = use_signal(Vec::<LogLine>::new);
    let ralph_lines = use_signal(Vec::<LogLine>::new);
    let mut paused = use_signal(|| false);
    let mut filter = use_signal(String::new);

    let task_id_value = id.0.clone();
    let workdir_signal = use_context::<state::Workdir>().signal();

    let _ = use_future(move || {
        let wd = workdir_signal.cloned();
        let tid = task_id_value.clone();
        let mut tel_buf = telemetry_lines;
        let mut ral_buf = ralph_lines;
        let paused = paused;

        async move {
            loop {
                if *paused.read() {
                    poll_sleep(POLL_INTERVAL_MS).await;
                    continue;
                }

                let tel_cursor = next_cursor(&tel_buf.read());
                match task_log_tail_telemetry(wd.clone(), tel_cursor).await {
                    Ok(new_lines) => append_capped(&mut tel_buf, new_lines),
                    Err(e) => {
                        eprintln!("task_log telemetry fetch error: {e:?}");
                    }
                }

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
        div {
            style: "padding:24px;",
            // Header.
            div {
                style: "display:flex;flex-wrap:wrap;align-items:baseline;justify-content:space-between;gap:12px;margin-bottom:20px;",
                div {
                    style: "display:flex;align-items:baseline;gap:12px;",
                    Link {
                        to: Route::TaskDetail { id: id.clone() },
                        class: "alps-mono",
                        style: "font-size:11px;color:var(--dim);text-decoration:none;",
                        "← "
                        span { "{id}" }
                    }
                    h1 {
                        class: "alps-mono",
                        style: "font-size:24px;font-weight:600;margin:0;color:var(--text);",
                        "Log"
                    }
                }
                div {
                    style: "display:flex;align-items:center;gap:8px;",
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
            // Dual-pane layout per deck §04.
            div {
                style: "display:grid;grid-template-columns:1fr 1fr;gap:18px;",
                LogPane {
                    label: "Workdir orchestrator log",
                    subtitle: "<workdir>/.alps-telemetry.log · shared across all tasks",
                    hint: "Per-task filter not yet available — elog! does not tag lines with task_id.",
                    lines: tel_lines,
                    filter_query: query.clone(),
                    max_lines: MAX_BUFFERED_LINES,
                    is_paused,
                }
                LogPane {
                    label: "Per-task Ralph/Codex activity",
                    subtitle: "<workdir>/tasks/<id>/implementation/ralph/.ralph-stderr.log",
                    hint: "Only meaningful while the task is in [implement] phase.",
                    lines: ral_lines,
                    filter_query: query,
                    max_lines: MAX_BUFFERED_LINES,
                    is_paused,
                }
            }
        }
    }
}

use crate::routes::Route;

/// Classify a log line into an info/warn/ok/err CSS class based on
/// substring matches in the line text. The deck §04 mockup shows the
/// same four colors for log levels.
fn classify_line(text: &str) -> &'static str {
    let lower = text.to_ascii_lowercase();
    if lower.contains("error") || lower.contains("fail") || lower.contains("panic") {
        "lv-err"
    } else if lower.contains("warn") || lower.contains("retry") || lower.contains("deprecat") {
        "lv-warn"
    } else if lower.contains("pass") || lower.contains("ok") || lower.contains("success") || lower.contains("attached") {
        "lv-ok"
    } else {
        "lv-info"
    }
}

#[component]
fn LogPane(
    label: &'static str,
    subtitle: &'static str,
    hint: &'static str,
    lines: Vec<LogLine>,
    filter_query: String,
    max_lines: usize,
    is_paused: bool,
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
        section {
            class: "log-pane",
            div {
                class: "log-toolbar",
                div {
                    style: "display:flex;flex-direction:column;gap:2px;min-width:0;flex:1;",
                    span {
                        class: "alps-mono",
                        style: "font-size:12px;color:var(--text);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;",
                        "{label}"
                    }
                    span {
                        class: "log-path",
                        "{subtitle}"
                    }
                }
                if is_paused {
                    span { class: "log-switch-off", title: "Paused" }
                } else {
                    span { class: "log-switch", title: "Polling" }
                }
            }
           div {
               class: "log-filter",
               // Filter input lives in the page header (shared across
               // both panes). The pane's filter row just shows the
               // "showing X of Y (cap Z)" count — the deck §04 mockup
               // has the filter input per-pane, but a single shared
               // filter is a saner UX (typing in one pane filters
               // both). The FilterInput above the dual-pane grid
               // writes to the shared `filter` signal.
               span {
                   "showing {filtered.len()} of {lines.len()} (cap {max_lines})"
               }
           }
            if filtered.is_empty() {
                div {
                    style: "padding:18px 20px;background:var(--ink);",
                    p {
                        class: "alps-sans",
                        style: "font-size:12px;color:var(--faint);font-style:italic;margin:0;",
                        "{hint}"
                    }
                }
            } else {
                pre {
                    class: "log-lines",
                    for line in filtered.iter() {
                        div {
                            span {
                                class: "ts",
                                style: "margin-right:10px;",
                                "{line.line_no:>4} "
                            }
                            span {
                                class: classify_line(&line.text),
                                "{line.text}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn PauseToggle(paused: bool, on_toggle: EventHandler<MouseEvent>) -> Element {
    rsx! {
        button {
            r#type: "button",
            class: "btn-ghost",
            style: "padding:6px 10px;font-size:11px;",
            onclick: move |evt| on_toggle.call(evt),
            title: if paused { "Resume polling" } else { "Pause polling — buffer freezes" },
            if paused { "▶ Resume" } else { "⏸ Pause" }
        }
    }
}

#[component]
fn FilterInput(query: String, on_input: EventHandler<String>) -> Element {
    rsx! {
        input {
            r#type: "search",
            class: "alps-mono",
            style: "background:transparent;border:1px solid var(--hair-soft);border-radius:6px;color:var(--dim);font-size:11px;width:140px;padding:5px 8px;outline:none;",
            placeholder: "filter…",
            value: "{query}",
            oninput: move |evt| on_input.call(evt.value()),
        }
    }
}

fn append_capped(buf: &mut Signal<Vec<LogLine>>, new_lines: Vec<LogLine>) {
    buf.with_mut(|b| {
        b.extend(new_lines);
        if b.len() > MAX_BUFFERED_LINES {
            let excess = b.len() - MAX_BUFFERED_LINES;
            b.drain(0..excess);
        }
    });
}

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
        let buf: Vec<LogLine> = (500..1500).map(|i| make_line(i, "x")).collect();
        assert_eq!(buf.len(), 1000, "buf should be 1000 lines (capped)");
        assert_eq!(next_cursor(&buf), 1500, "next cursor must be 1500, NOT 1000");
    }

    #[test]
    fn classify_line_recognizes_levels() {
        assert_eq!(classify_line("tick task-042 phase=implement"), "lv-info");
        assert_eq!(classify_line("worker attached pid 88214"), "lv-ok");
        assert_eq!(classify_line("retry: git fetch origin (1/3)"), "lv-warn");
        assert_eq!(classify_line("error: spawn failed"), "lv-err");
        assert_eq!(classify_line("cargo test: 1 flaky retry"), "lv-warn");
        assert_eq!(classify_line("cargo check passed"), "lv-ok");
    }

    /// SSR contract for M3b: the page must render both pane labels +
    /// the Pause button in the SSR'd HTML so the verify-script's
    /// #5g acceptance criteria pass without hydration.
    #[test]
    fn task_log_ssr_shows_both_pane_labels_and_pause_button() {
        use crate::pages::TaskLog;
        use crate::state::provide_workdir;

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
        // Pause button label.
        assert!(
            html.contains("Pause"),
            "Pause button should render in SSR: {html}"
        );
    }
}
