//! NotFound catch-all page (`/:..segments`).
//!
//! US-003 ships a placeholder that surfaces the unmatched path so users see
//! what URL they actually hit. The catch-all is REQUIRED per the dioxus-
//! router skill — without it the router renders nothing on unmatched URLs
//! (per SPEC §5 footnote + dioxus_router::Routable SITE_MAP behavior).
//!
//! `segments` is the spread-captured `Vec<String>` of all path segments that
//! failed to match any other route. We render it joined with "/" so the
//! 404 page shows "/foo/bar/baz" rather than three bare strings.

use dioxus::prelude::*;

#[component]
pub fn NotFound(segments: Vec<String>) -> Element {
    let path = if segments.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", segments.join("/"))
    };
    rsx! {
        div { class: "p-4 sm:p-6 lg:p-8 space-y-4",
            h1 { class: "text-2xl font-semibold text-slate-800", "Not found" }
            div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm space-y-2",
                p { class: "text-sm text-slate-700",
                    "No route matched the path "
                    span { class: "font-mono text-slate-500", "{path}" }
                    "."
                }
                p { class: "text-sm text-slate-700", "NotFound — coming in v2" }
            }
        }
    }
}
