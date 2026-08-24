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
//! ## Responsive behavior
//!
//! Per DESIGN.md §3 + SPEC §5:
//!
//! | Breakpoint | Width    | Nav shape                                       |
//! |------------|----------|-------------------------------------------------|
//! | (default)  | 0+       | Single column; nav links COLLAPSE to a hamburger button (`< sm:`) |
//! | `sm:`      | >= 640px | Nav is horizontal (links visible)               |
//!
//! Tailwind's responsive variants drive the visibility — no JS-side
//! `window.matchMedia` branching. The hamburger button is `sm:hidden`
//! (visible only < sm), and the inline nav is `hidden sm:flex`
//! (visible only on sm+).
//!
//! ## Interactivity (intentionally NOT wired in US-003)
//!
//! The hamburger button has no click handler in this story. Toggling a
//! mobile menu open/closed requires a `Signal<bool>` + `use_signal` that
//! the SPEC defers until US-006+ (when NavState context is introduced).
//! For US-003 the hamburger is decorative: it correctly appears on
//! < sm and disappears on sm+ — that's the load-bearing responsive
//! behavior — but pressing it does nothing yet.
//!
//! ## Accessibility
//!
//! The hamburger button carries `aria-label="Open menu"` so screen
//! readers have a name even though the icon is text-only. The hidden-on-
//! desktop nav carries `aria-label="Primary"` for the same reason.
//!
//! ## Why no NavState context yet
//!
//! SPEC §6.6 / acceptance criteria mention a `NavState` context that the
//! NavBar reads via `use_context::<NavState>()`. That context tracks the
//! active workdir + orchestrator API URL + MINIMAX_API_KEY indicator.
//! None of those settings surface exist yet — US-008 confirms Settings is
//! a stub for the smoke — so the NavBar in US-003 hard-codes the brand
//! line and the three primary nav links. A follow-up story wires
//! `use_context_provider(NavState::default)` in `App` and adds a workdir
//! picker + version chip.

use dioxus::prelude::*;
use dioxus::router::components::{Link, Outlet};

use crate::routes::Route;

/// Responsive top-bar layout.
///
/// Layout structure (outer -> inner):
///
/// ```text
/// <div min-h-screen flex flex-col>
///   <header sticky top-0 z-10 bg-white border-b border-slate-200 shadow-sm>
///     <div flex items-center justify-between p-4>
///       <brand>                        # always visible
///         <Link to=Dashboard>ALPS</Link>
///         <span> v0.1.0 </span>
///       </brand>
///       <nav hidden sm:flex>           # visible only on sm+
///         <Link to=Dashboard>Dashboard</Link>
///         <Link to=NewTask>New task</Link>
///         <Link to=Settings>Settings</Link>
///       </nav>
///       <button sm:hidden>             # visible only < sm
///         aria-label="Open menu"
///         hamburger glyph
///       </button>
///     </div>
///   </header>
///   <main flex-1>
///     <Outlet::<Route> />              # the matched child route renders here
///   </main>
/// </div>
/// ```
#[component]
pub fn NavBar() -> Element {
    rsx! {
        div { class: "min-h-screen flex flex-col bg-slate-50",
            header { class: "sticky top-0 z-10 bg-white border-b border-slate-200 shadow-sm",
                div { class: "flex items-center justify-between p-4",
                    div { class: "flex items-baseline gap-3",
                        Link {
                            to: Route::Dashboard {},
                            class: "text-lg font-semibold text-slate-800 hover:text-slate-900",
                            "ALPS"
                        }
                        span { class: "text-xs text-slate-500", "v0.1.0" }
                    }
                    nav {
                        class: "hidden sm:flex items-center gap-2",
                        "aria-label": "Primary",
                        Link {
                            to: Route::Dashboard {},
                            class: "px-3 py-2 rounded-md text-sm font-medium text-slate-700 hover:bg-slate-100",
                            "Dashboard"
                        }
                        Link {
                            to: Route::NewTask {},
                            class: "px-3 py-2 rounded-md text-sm font-medium text-slate-700 hover:bg-slate-100",
                            "New task"
                        }
                        Link {
                            to: Route::Settings {},
                            class: "px-3 py-2 rounded-md text-sm font-medium text-slate-700 hover:bg-slate-100",
                            "Settings"
                        }
                    }
                    button {
                        // sm:hidden → visible below 640px only.
                        // `aria-label` + `aria-expanded` give the button a name
                        // for assistive tech even though it has no label text.
                        r#type: "button",
                        class: "sm:hidden inline-flex items-center justify-center p-2 rounded-md text-slate-700 hover:bg-slate-100",
                        "aria-label": "Open menu",
                        "aria-expanded": "false",
                        // Unicode hamburger keeps the v1 build zero-dependency.
                        // A future story swaps this for an inline SVG once the
                        // mobile menu actually opens (needs a Signal<bool>).
                        "☰"
                    }
                }
            }
            main { class: "flex-1",
                Outlet::<Route> {}
            }
        }
    }
}
