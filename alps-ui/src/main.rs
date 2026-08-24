//! ALPS UI — entry point.
//!
//! See `SPEC.md` and `DESIGN.md` in the repo root for the full design.
//! This is the first-cut scaffold (US-001 + US-002): a Dioxus 0.7 fullstack
//! app that ships web / desktop / mobile from a single `rsx!{}` tree,
//! backed by read-side server functions over the `alps-core` domain types
//! re-exported from `crate::domain`.

use dioxus::prelude::*;

mod domain;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        div { "alps-ui — coming soon" }
    }
}
