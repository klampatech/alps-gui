//! Presentational components (DESIGN.md §4).
//!
//! These are the named components that pages compose into the final UI. They
//! are stateless fragments with typed props — they take data in and return a
//! single `Element`. Unlike layouts (which wrap an `<Outlet>` and are
//! referenced by `#[layout(...)]` on the `Route` enum), components live
//! inside a page's `rsx!{}` tree and are rendered by call site.
//!
//! ## What this file exports
//!
//! - `StatusPill` — color-coded badge for one of 9 `TaskState` variants.
//! - `StoryCard` — one `UserStory` row in the TaskDetail Plan tab.
//! - `FindingCard` — one entry in a Review's findings list (severity pill).
//! - `AssertionCard` — one entry in a Review's assertions list.
//! - `ReceiptCard` — the final `Receipts` summary for a Done task.
//! - `ResponsiveGrid` — 1-col-default, 3-col-on-`lg:` wrapper for the
//!   Dashboard's three sections (task list, new-task form, recent log).
//!
//! ## When this file grows
//!
//! - US-005 will use `StatusPill` + `ResponsiveGrid` + (later) `StoryCard`
//!   to render the Dashboard fixture list and TaskDetail plan list.
//! - US-006 will hand these components real server-function data (via
//!   `use_resource`), but the components themselves stay presentation-only
//!   — they take typed props and emit `rsx!{}`.
//!
//! ## `#[allow(dead_code)]` and `#[allow(unused_imports)]`
//!
//! US-004 lands BEFORE its only consumer (US-005's dashboard). Rust's
//! `dead_code` lint flags exported `pub fn` items in a binary crate as
//! unused when no caller imports them yet, and `unused_imports` flags
//! every `pub use` line that nothing downstream currently names. The
//! module-level allows drop in US-005 when the Dashboard's `rsx!{}`
//! references each component. Same pattern as `domain.rs`'s
//! `#[allow(unused_imports)]` — keep one module-level annotation,
//! then strip it when the consumer lands.
#![allow(dead_code, unused_imports)]

mod responsive_grid;

mod assertion_card;
mod finding_card;
mod receipt_card;
mod status_pill;
mod story_card;

pub use assertion_card::AssertionCard;
pub use finding_card::FindingCard;
pub use receipt_card::ReceiptCard;
pub use responsive_grid::ResponsiveGrid;
pub use status_pill::StatusPill;
pub use story_card::StoryCard;
