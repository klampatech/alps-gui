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
//! - `StatusPill` — color-coded badge for a `TaskState` (9-variant exhaustive
//!   match + 6-style visual collapse per docs/alps-gui-redesign-deck.html
//!   §01). **Consumed by Dashboard, TaskDetail.**
//! - `Rail` — the 5-node pipeline rail (Plan/Implement/Review/Judge/Done),
//!   two variants: `FullRail` for TaskDetail's page header, `MiniRail` for
//!   Dashboard task cards. **Consumed by Dashboard, TaskDetail.**
//! - `StoryCard` — one `UserStory` row in the TaskDetail Plan tab.
//! - `FindingCard` — one entry in a Review's findings list.
//! - `AssertionCard` — one entry in a Review's assertions list.
//! - `ReceiptCard` — the final `Receipts` summary for a Done task.
//! - `ResponsiveGrid` — 1-col-default, 3-col-on-`lg:` wrapper (kept for
//!   backwards compatibility; the Control Rail redesign uses a 2-col
//!   Dashboard grid that doesn't need the responsive-3-col wrapper).
//!
//! ## `#[allow(unused_imports)]` for the unconsumed re-exports
//!
//! The TaskDetail sub-cards (`StoryCard` / `FindingCard` /
//! `AssertionCard` / `ReceiptCard`) stay as-is for the redesign — they
//! still render inside the Plan / Review / Receipts accordions. Per
//! status_pill.rs the `match` on `TaskState` is exhaustive over all 9
//! variants; the re-exports stay live.
#![allow(unused_imports)]

mod rail;

mod assertion_card;
mod finding_card;
mod receipt_card;
mod responsive_grid;
mod status_pill;
mod story_card;

pub use assertion_card::AssertionCard;
pub use finding_card::FindingCard;
pub use rail::{phase_for_state, FullRail, MiniRail};
pub use receipt_card::ReceiptCard;
pub use responsive_grid::ResponsiveGrid;
pub use status_pill::StatusPill;
pub use story_card::StoryCard;
