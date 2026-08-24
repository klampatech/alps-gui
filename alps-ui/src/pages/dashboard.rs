//! Dashboard page (`/`).
//!
//! US-003 ships a placeholder here. US-005 replaces the body with the real
//! `ResponsiveGrid` + `TaskSummary` fixture list (3 sections: NewTask form +
//! task list + recent activity). The outer page shell — `p-4 sm:p-6 lg:p-8`
//! padding, the `h1` title — is preserved by US-005 so the title and
//! padding stay consistent across the placeholder-to-real transition.

use dioxus::prelude::*;

#[component]
pub fn Dashboard() -> Element {
    rsx! {
        div { class: "p-4 sm:p-6 lg:p-8 space-y-4",
            h1 { class: "text-2xl font-semibold text-slate-800", "Dashboard" }
            p { class: "text-sm text-slate-600",
                "Task list, NewTask form, and recent activity will live here in US-005."
            }
            div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm",
                p { class: "text-sm text-slate-700", "Dashboard — coming in v2" }
            }
        }
    }
}
