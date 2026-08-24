//! `FindingCard` — one entry in a Review's `findings` list (DESIGN.md §4).
//!
//! Renders the finding's severity pill, description, and evidence. The
//! severity palette matches DESIGN.md §2's pill table:
//!
//! | Severity   | Tailwind class |
//! |------------|----------------|
//! | `Info`     | `bg-slate-400` |
//! | `Warning`  | `bg-amber-500` |
//! | `Error`    | `bg-rose-500`  |
//! | `Critical` | `bg-rose-700`  |
//!
//! ## Card chrome
//!
//! Same `rounded-lg border border-slate-200 bg-white p-4 shadow-sm`
//! pattern as every other card. The description is `text-sm text-slate-800`
//! (the user-facing headline of the finding), and the evidence is `text-xs
//! text-slate-500 italic` so the visual hierarchy lines up with the Review
//! tab's other sub-components (AssertionCard, etc.).
//!
//! ## Accessibility
//!
//! Severity pill carries `role="status"` so assistive tech announces the
//! severity when the finding enters the DOM.

use dioxus::prelude::*;
use crate::domain::{Finding, Severity};

#[component]
pub fn FindingCard(finding: Finding) -> Element {
    let (label, bg) = match finding.severity {
        Severity::Info => ("Info", "bg-slate-400"),
        Severity::Warning => ("Warning", "bg-amber-500"),
        Severity::Error => ("Error", "bg-rose-500"),
        Severity::Critical => ("Critical", "bg-rose-700"),
    };
    // Hoist the severity debug-name out of the rsx!{} tree because
    // the Dioxus format-string parser trips on nested braces inside
    // a `"{...}"` interpolation (it sees `{:?}` as an unmatched
    // closing brace). Building the string in a let-binding keeps the
    // outer Dioxus interpolation simple.
    let severity_dbg = format!("{:?}", finding.severity);

    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm space-y-2",
            div { class: "flex items-center gap-2",
                span {
                    class: "rounded-full px-2.5 py-0.5 text-xs font-medium text-white {bg}",
                    role: "status",
                    "{label}"
                }
                span { class: "text-xs text-slate-500 font-mono", "{severity_dbg}" }
            }
            p { class: "text-sm text-slate-800", "{finding.description}" }
            p { class: "text-xs text-slate-500 italic", "{finding.evidence}" }
        }
    }
}
