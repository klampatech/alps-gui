//! NotFound catch-all page (`/:..segments`).
//!
//! Control Rail redesign: dark-themed 404 page with the failed path
//! displayed in mono amber (per deck §04 visual treatment).

use dioxus::prelude::*;
use crate::routes::Route;

#[component]
pub fn NotFound(segments: Vec<String>) -> Element {
    let path = if segments.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", segments.join("/"))
    };
    rsx! {
        div {
            style: "padding:64px 24px;text-align:center;",
            p {
                class: "alps-mono",
                style: "font-size:48px;color:var(--faint);margin:0 0 8px 0;",
                "404"
            }
            p {
                class: "alps-mono",
                style: "font-size:13px;color:var(--amber);margin:0 0 24px 0;",
                "{path}"
            }
            p {
                style: "font-size:13px;color:var(--dim);margin:0 0 24px 0;",
                "No route matched this path."
            }
            Link {
                to: Route::Dashboard {},
                class: "btn-ghost",
                "← Back to Dashboard"
            }
        }
    }
}
