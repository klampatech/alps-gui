//! Settings page (`/settings`).
//!
//! US-003 ships a placeholder. Per US-008 acceptance criteria #5: the
//! Settings page is a STUB for the entire smoke scope. The real page
//! (NavState context + workdir picker + MINIMAX_API_KEY display) lands
//! after US-006. The copy below is the load-bearing "no real auth /
//! settings yet" signal — anyone landing on /settings sees it.

use dioxus::prelude::*;

#[component]
pub fn Settings() -> Element {
    rsx! {
        div { class: "p-4 sm:p-6 lg:p-8 space-y-4",
            h1 { class: "text-2xl font-semibold text-slate-800", "Settings" }
            div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm",
                p { class: "text-sm text-slate-700", "Settings — coming in v2" }
            }
        }
    }
}
