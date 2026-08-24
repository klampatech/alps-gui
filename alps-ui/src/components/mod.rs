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
//!   **Consumed by US-005's Dashboard.**
//! - `StoryCard` — one `UserStory` row in the TaskDetail Plan tab.
//!   Lands in US-006+ (TaskDetail render).
//! - `FindingCard` — one entry in a Review's findings list (severity pill).
//!   Lands in US-006+ (TaskDetail Review tab).
//! - `AssertionCard` — one entry in a Review's assertions list.
//!   Lands in US-006+ (TaskDetail Review tab).
//! - `ReceiptCard` — the final `Receipts` summary for a Done task.
//!   Lands in US-006+ (TaskDetail Receipts tab).
//! - `ResponsiveGrid` — 1-col-default, 3-col-on-`lg:` wrapper.
//!   **Consumed by US-005's Dashboard.**
//!
//! ## `#[allow(unused_imports)]` for the unconsumed re-exports
//!
//! US-005 lands `ResponsiveGrid` + `StatusPill` into the Dashboard.
//! The other four components (`StoryCard`, `FindingCard`,
//! `AssertionCard`, `ReceiptCard`) are still unused until US-006+ wires
//! them into TaskDetail / TaskLog / TaskDiff. To suppress the
//! `unused_imports` lint on those four re-exports without keeping the
//! dead-code/unused-imports allow on the whole module, each unconsumed
//! re-export carries an inline `#[allow(unused_imports)]`. Strip those
//! once US-006+ adds the consumer.
#![allow(unused_imports)]

mod responsive_grid;

mod assertion_card;
mod finding_card;
mod receipt_card;
mod status_pill;
mod story_card;

#[allow(unused_imports)]
pub use assertion_card::AssertionCard;
#[allow(unused_imports)]
pub use finding_card::FindingCard;
#[allow(unused_imports)]
pub use receipt_card::ReceiptCard;
pub use responsive_grid::ResponsiveGrid;
pub use status_pill::StatusPill;
#[allow(unused_imports)]
pub use story_card::StoryCard;
