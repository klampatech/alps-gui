//! TaskLog page (`/tasks/:id/log`).
//!
//! US-003 ships a placeholder that displays the typed TaskId path segment.
//! The real implementation is a Server-Sent Events tail over the orchestrator
//! elog! lines. Per US-008 that is explicitly OUT OF SCOPE for the smoke;
//! the placeholder stays until a follow-up story wires `task_log_stream`.
//!
//! The placeholder still proves the route + typed segment work. Visit
//! `/tasks/2026-08-23T192049-abcdef01/log` and the heading shows the ID.

use dioxus::prelude::*;

use crate::domain::TaskId;

#[component]
pub fn TaskLog(id: TaskId) -> Element {
    rsx! {
        div { class: "p-4 sm:p-6 lg:p-8 space-y-4",
            h1 { class: "text-2xl font-semibold text-slate-800",
                "Log for "
                span { class: "font-mono text-base text-slate-500", "{id}" }
            }
            div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm",
                p { class: "text-sm text-slate-700", "TaskLog — coming in v2" }
            }
        }
    }
}
