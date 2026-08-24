//! Layout components — components that wrap one or more `Route` variants
//! via the `#[layout(Component)]` attribute on the `Routable` enum.
//!
//! Per dioxus_router conventions: every layout component MUST render an
//! `<Outlet::<Route> />` somewhere in its body so the matched child route's
//! page component appears inside the layout shell. See
//! `dioxus_router::components::outlet::Outlet` for the trait bound
//! (`Routable + Clone`) and the `LayoutContext` resolution algorithm.
//!
//! Layouts live under `src/layouts/` (not `src/components/`) per DESIGN.md
//! §4 convention. Components (`StatusPill`, `StoryCard`, etc.) live under
//! `src/components/` and are stateless fragments that take typed props.
//! Layouts are higher-level: they own the page chrome and are referenced by
//! `Route::#[layout(...)]` attributes, not composed into a page manually.

mod nav;

pub use nav::NavBar;
