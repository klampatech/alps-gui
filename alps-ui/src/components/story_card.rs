//! `StoryCard` — one `UserStory` row in the TaskDetail Plan tab (DESIGN.md §4).
//!
//! Renders the story's title, description, identifier, and an acceptance-
//! criteria checklist. When `passes` is supplied, shows a Pass/Pending
//! pill so the user can tell at a glance which stories are still open.
//!
//! ## Layout (DESIGN.md §2)
//!
//! - `rounded-lg border border-slate-200 bg-white p-4 shadow-sm` card chrome
//!   (the same pattern every other card uses)
//! - Title `text-base font-medium text-slate-800`
//! - Description `text-sm text-slate-600`
//! - Acceptance criteria list with `space-y-1` and a 1.5×1.5 slate dot
//!   bullet to match DESIGN.md §4's reference snippet
//!
//! ## `passes` semantics
//!
//! `Option<bool>` so the caller can either supply a verdict or omit the
//! pill entirely. `Some(true)` renders a green `bg-emerald-500` "Pass"
//! pill; `Some(false)` renders a slate `bg-slate-400` "Pending" pill; `None`
//! renders nothing (e.g. when the orchestrator hasn't reported yet).
//!
//! ## `key` on the criteria loop
//!
//! Per acceptance criterion: every `for` loop over a list uses `key` on
//! the inner element. The acceptance-criterion text isn't unique on its
//! own, so we pair it with the story's index for stable reconciliation.
use dioxus::prelude::*;
use crate::domain::UserStory;
#[component]
pub fn StoryCard(story: UserStory, passes: Option<bool>) -> Element {
    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm space-y-2",
            div { class: "flex items-start justify-between gap-2",
                h3 { class: "text-base font-medium text-slate-800", "{story.title}" }
                {passes.map(|p| {
                    let pill_class = if p {
                        "rounded-full px-2.5 py-0.5 text-xs font-medium text-white bg-emerald-500"
                    } else {
                        "rounded-full px-2.5 py-0.5 text-xs font-medium text-white bg-slate-400"
                    };
                    let pill_label = if p { "Pass" } else { "Pending" };
                    rsx! {
                        span { class: "{pill_class}", "{pill_label}" }
                    }
                })}
            }
            div { class: "flex items-center gap-2 text-xs text-slate-500 font-mono",
                "ID: {story.id.0}"
                span { class: "text-slate-300", "·" }
                "priority #{story.priority}"
            }
            p { class: "text-sm text-slate-600", "{story.description}" }
            ul { class: "space-y-1",
                for (idx, ac) in story.acceptance_criteria.iter().enumerate() {
                    li {
                        key: "{story.id.0}-{idx}",
                        class: "flex items-start gap-2 text-sm text-slate-700",
                        span { class: "mt-1 h-1.5 w-1.5 rounded-full bg-slate-400" }
                        span { "{ac}" }
                    }
                }
            }
        }
    }
}
