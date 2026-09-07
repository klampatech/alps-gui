//! Responsive top-bar layout — wraps every `Route` variant per SPEC §5.
//!
//! ## What this file is
//!
//! `NavBar` is the layout component referenced by `#[layout(NavBar)]` on
//! the `Route` enum (see `src/routes.rs`). Every page renders INSIDE this
//! layout via the `<Outlet::<Route> />` call at the bottom of the function
//! body — without that outlet, the router has no place to render the page
//! content and `App` would show only the navbar.
//!
//! ## Control Rail redesign (deck §02 / §06)
//!
//! Per docs/alps-gui-redesign-deck.html §02, the navbar is dark, mono,
//! and shows the brand `ALPS v0.1.0` (amber accent on the version), the
//! primary nav links, and the workdir path on the right. The page
//! chrome (`bg-ink` via `var(--ink)`) is set on the outer container so
//! every page inherits it.
//!
//! ## Mobile menu (deferred)
//!
//! The deck omits the hamburger menu — desktop nav only. We keep the
//! `sm:hidden` button as a placeholder for a follow-up PR. The button
//! is decorative on < sm (no click handler).
//!
//! ## Workdir chip
//!
//! The navbar reads the workdir from the shared `state::Workdir`
//! context and renders it as a mono `<span>` on the right. Settings'
//! Save handler updates the context, which re-renders this layout.
//!
//! ## Why `var(--ink)` instead of a Tailwind arbitrary value
//!
//! The NavBar's outer container is a single block — the ink bg applies
//! to a div the user sees on every page. Setting it via a CSS custom
//! property (defined in `assets/main.css`) means we don't need to ship
//! a new Tailwind class for every redesign color. The other dark
//! surfaces (`var(--panel2)`, `var(--hair)`) work the same way.

use dioxus::prelude::*;
use dioxus::router::components::{Link, Outlet};

use crate::routes::Route;
use crate::state;

#[component]
pub fn NavBar() -> Element {
    // The workdir is the reactive thing — read it once at render time,
    // subscribe via `cloned()` so changes from the Settings page's Save
    // button update the chip. (Same pattern as M4-proper's reactivity.)
    let workdir_signal = use_context::<state::Workdir>().signal();

    rsx! {
        div {
            class: "min-h-screen flex flex-col",
            style: "background:var(--ink);color:var(--text);",
            header {
                class: "sticky top-0 z-10",
                style: "background:var(--panel);border-bottom:1px solid var(--hair);",
                div {
                    class: "flex items-center justify-between gap-4 px-4 sm:px-6 lg:px-8 py-3",
                    // Brand — mono + amber accent on the version chip.
                    div {
                        class: "flex items-baseline gap-3",
                        Link {
                            to: Route::Dashboard {},
                            style: "font-family:var(--mono);font-weight:600;font-size:14px;letter-spacing:.06em;color:var(--text);text-decoration:none;",
                            "ALPS "
                            span {
                                style: "color:var(--amber);",
                                "v0.1.0"
                            }
                        }
                    }
                    // Nav links — desktop only (deck §02 shows only desktop).
                    // The `sm:flex` activates >= 640px; below that the
                    // hamburger placeholder is the only visible element.
                    nav {
                        class: "hidden sm:flex items-center gap-6",
                        style: "font-family:var(--mono);font-size:12px;",
                        "aria-label": "Primary",
                        Link {
                            to: Route::Dashboard {},
                            style: "color:var(--dim);text-decoration:none;",
                            active_class: "color:var(--text);",
                            "Dashboard"
                        }
                        Link {
                            to: Route::NewTask {},
                            style: "color:var(--dim);text-decoration:none;",
                            active_class: "color:var(--text);",
                            "New task"
                        }
                        Link {
                            to: Route::Settings {},
                            style: "color:var(--dim);text-decoration:none;",
                            active_class: "color:var(--text);",
                            "Settings"
                        }
                    }
                    // Right side: workdir chip + hamburger placeholder.
                    div {
                        class: "flex items-center gap-3",
                        // Workdir chip — mono + faint text. Truncated
                        // with text-overflow:ellipsis on narrow viewports
                        // (the chip is decorative on mobile).
                        span {
                            class: "alps-mono",
                            style: "font-size:11px;color:var(--faint);max-width:36ch;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;display:inline-block;",
                            title: "{workdir_signal.cloned()}",
                            "{workdir_signal.cloned()}"
                        }
                        button {
                            // sm:hidden → visible below 640px only.
                            // `aria-label` + `aria-expanded` give the
                            // button a name for assistive tech even
                            // though it has no label text. The deck
                            // omits the hamburger; this is a placeholder
                            // for the follow-up mobile-menu story.
                            r#type: "button",
                            class: "sm:hidden inline-flex items-center justify-center p-2 rounded-md",
                            style: "color:var(--dim);",
                            "aria-label": "Open menu",
                            "aria-expanded": "false",
                            "☰"
                        }
                    }
                }
            }
            main {
                class: "flex-1",
                Outlet::<Route> {}
            }
        }
    }
}
