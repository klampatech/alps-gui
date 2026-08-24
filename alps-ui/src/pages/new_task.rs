//! NewTask page (`/tasks/new`).
//!
//! US-003 ships a placeholder. The real form (textarea + Submit + a
//! `task_run` server-function call) lands alongside US-005/US-006. Until
//! then, the placeholder lets the NavBar link resolve to a real page.

use dioxus::prelude::*;

#[component]
pub fn NewTask() -> Element {
    rsx! {
        div { class: "p-4 sm:p-6 lg:p-8 space-y-4",
            h1 { class: "text-2xl font-semibold text-slate-800", "New task" }
            div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm",
                p { class: "text-sm text-slate-700", "NewTask — coming in v2" }
            }
        }
    }
}
