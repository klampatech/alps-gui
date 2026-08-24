//! Placeholder page components — one per `Route` variant.
//!
//! Each component lives in its own submodule so its signature (`fn
//! Dashboard()` etc.) matches the `Routable` enum's variant names 1:1 — the
//! `Routable` derive looks up components by the variant's bare name.
//!
//! Per SPEC §5 + US-003 acceptance: every page (except `Dashboard`, which
//! US-005 will fill in with real fixtures) renders a `<p>{route_name} —
//! coming in v2</p>` placeholder inside a card. The page-level padding comes
//! from the `p-4 sm:p-6 lg:p-8` shell that DESIGN.md §2 specifies for every
//! page; the inner card uses `rounded-lg border border-slate-200 bg-white
//! p-4 shadow-sm` per the same section.
//!
//! ## When this file grows
//!
//! - `dashboard.rs` will gain the `ResponsiveGrid` + fixture list layout
//!   in US-005.
//! - `task_detail.rs`, `task_log.rs`, and `task_diff.rs` will grow into
//!   real screens once the read-side server functions land (US-006+).
//! - `settings.rs` stays a stub for the entire smoke scope per US-008 —
//!   `Settings coming in v2` is the load-bearing copy until a follow-up
//!   story adds the `NavState` context + workdir picker.
//!
//! `#[allow(dead_code)]` is NOT used here because the `Routable` derive
//! generates `Route::Dashboard {}.render(...)` call sites that reference
//! each component as soon as `App` mounts `<Router<Route>>`, so the
//! "unused" lint never fires once the router is wired in `main.rs`.

mod dashboard;
mod new_task;
mod not_found;
mod settings;
mod task_detail;
mod task_diff;
mod task_log;

pub use dashboard::Dashboard;
pub use new_task::NewTask;
pub use not_found::NotFound;
pub use settings::Settings;
pub use task_detail::TaskDetail;
pub use task_diff::TaskDiff;
pub use task_log::TaskLog;
