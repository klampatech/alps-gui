//! `AssertionCard` — one entry in a Review's `assertions` list (DESIGN.md §4).
//!
//! Renders an assertion as a checklist row: a check / cross glyph, the
//! criterion text, and (when supplied) the evidence the assertion relied
//! on.
//!
//! ## Visual signal
//!
//! Per DESIGN.md §4 the glyph is `[x]` / `[ ]` style. We use Unicode
//! `✓` (heavy check mark, green when passed) and `✗` (heavy cross, rose
//! when failed). Color comes from Tailwind text utilities, NOT hex codes.
//!
//! ## Card chrome
//!
//! Same `rounded-lg border border-slate-200 bg-white p-4 shadow-sm`
//! pattern as every other card.
//!
//! ## Why a passed attribute (and not a checkable `Review`)
//!
//! The component is presentation-only — it renders one row of
//! already-evaluated data. The `Review` shape and assertion-evaluation
//! pipeline live in `alps-core`; the UI just receives a `Vec<Assertion>`
//! and renders it.
use dioxus::prelude::*;
use crate::domain::Assertion;
#[component]
pub fn AssertionCard(assertion: Assertion) -> Element {
    let (glyph, color) = if assertion.passed {
        ("✓", "text-emerald-500")
    } else {
        ("✗", "text-rose-500")
    };
    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm space-y-1",
            div { class: "flex items-start gap-2",
                span { class: "text-lg {color} leading-none", "{glyph}" }
                span { class: "text-sm text-slate-800", "{assertion.criterion}" }
            }
            p { class: "text-xs text-slate-500", "{assertion.evidence}" }
        }
    }
}
