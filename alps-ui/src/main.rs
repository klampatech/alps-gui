//! ALPS UI — entry point.
//!
//! See `SPEC.md` and `DESIGN.md` in the repo root for the full design.
//! This is the first-cut scaffold (US-001): a Dioxus 0.7 fullstack app
//! that ships web / desktop / mobile from a single `rsx!{}` tree.

use dioxus::prelude::*;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        div { "alps-ui — coming soon" }
    }
}
