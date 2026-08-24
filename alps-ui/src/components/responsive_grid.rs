//! `ResponsiveGrid` — 1-col-default, 3-col-on-`lg:` wrapper (DESIGN.md §3).
//!
//! Renders its `children` inside a `div` with the canonical Tailwind
//! grid classes. The Dashboard's three sections (task list / new-task
//! form / recent log) sit inside this grid so they share the same
//! responsive breakpoint behavior (DESIGN.md §5):
//!
//! ```text
//! < 1024px (default):  one column, three sections stacked top-to-bottom
//! ≥ 1024px (lg:):     three columns side-by-side
//! ```
//!
//! ## Why a `children: Element` parameter
//!
//! Dioxus 0.7 components compose their children at the call site. A
//! parent component passes each section as positional/keyword children
//! and `ResponsiveGrid` wraps them in the responsive grid. The rendered
//! HTML is identical to writing the grid classes directly on the
//! parent `<div>` — this component exists so the responsive shape is
//! named (DESIGN.md §4) and reusable across pages without copy-pasting
//! the class string.
//!
//! ## `gap-4 p-4`
//!
//! `gap-4` is DESIGN.md §3's "Stack gap" (1rem between cards). `p-4` is
//! the page padding at the default breakpoint — on `sm:` the `NavBar`
//! already provides larger padding via the page-level `sm:p-6 lg:p-8`
//! wrapper so this component's `p-4` only takes effect within its own
//! grid container.
use dioxus::prelude::*;
#[component]
pub fn ResponsiveGrid(children: Element) -> Element {
    rsx! {
        div { class: "grid grid-cols-1 lg:grid-cols-3 gap-4 p-4",
            {children}
        }
    }
}
