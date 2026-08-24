//! TaskDiff page (`/tasks/:id/diff`).
//!
//! US-003 ships a placeholder that displays the typed TaskId path segment.
//! The real implementation shells out to `git diff` over the alps/<id>
//! worktree and renders a unified-diff view. Per US-006 the read-side API
//! only covers tasks_list / task_get / task_run; `task_diff` is deferred
//! past the smoke so this placeholder stays.

use dioxus::prelude::*;

use crate::domain::TaskId;

#[component]
pub fn TaskDiff(id: TaskId) -> Element {
    rsx! {
        div { class: "p-4 sm:p-6 lg:p-8 space-y-4",
            h1 { class: "text-2xl font-semibold text-slate-800",
                "Diff for "
                span { class: "font-mono text-base text-slate-500", "{id}" }
            }
            div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm",
                p { class: "text-sm text-slate-700", "TaskDiff — coming in v2" }
            }
        }
    }
}
