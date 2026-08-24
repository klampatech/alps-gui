//! TaskDetail page (`/tasks/:id`).
//!
//! US-003 ships a placeholder that displays the typed `id: TaskId` path
//! segment so we can verify the router correctly parses + Display-renders
//! the segment end-to-end. US-004 adds the StoryCard / FindingCard /
//! AssertionCard / ReceiptCard components that will populate the body once
//! US-006 task_get server function feeds real TaskDetail data into the page.

use dioxus::prelude::*;

use crate::domain::TaskId;

#[component]
pub fn TaskDetail(id: TaskId) -> Element {
    rsx! {
        div { class: "p-4 sm:p-6 lg:p-8 space-y-4",
            h1 { class: "text-2xl font-semibold text-slate-800",
                "Task "
                span { class: "font-mono text-base text-slate-500", "{id}" }
            }
            div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm",
                p { class: "text-sm text-slate-700", "TaskDetail — coming in v2" }
            }
        }
    }
}
