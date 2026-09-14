//! NewTask page (`/tasks/new`).
//!
//! Control Rail redesign: the deck §02 shows the NewTask card living
//! inside the Dashboard's left column (not on its own route). The
//! `/tasks/new` route still exists for backwards compat (the nav link
//! resolves to it) but redirects to the Dashboard with a hint.
//!
//! Actually — the NewTask form lives in the Dashboard's left column
//! per deck §02. This `/tasks/new` route is a placeholder that the
//! NavBar link points to; clicking it just renders the Dashboard's
//! form. For v1 simplicity we keep the route alive and render a
//! minimalist placeholder.

use dioxus::prelude::*;
use crate::routes::Route;

#[component]
pub fn NewTask() -> Element {
    rsx! {
        div {
            style: "padding:24px;",
            div {
                class: "surface",
                style: "padding:24px;",
                h1 {
                    class: "alps-mono",
                    style: "font-size:24px;font-weight:600;margin:0 0 12px 0;color:var(--text);",
                    "New task"
                }
                p {
                    style: "font-size:13px;color:var(--dim);margin:0 0 16px 0;line-height:1.55;",
                    "The NewTask form lives on the Dashboard's left column (per the Control Rail redesign). Navigate back there to submit a task."
                }
                Link {
                    to: Route::Dashboard {},
                    class: "btn-amber",
                    "← Back to Dashboard"
                }
            }
        }
    }
}
