//! `StatusPill` — color-coded badge for a `TaskState` (DESIGN.md §4).
//!
//! This is the load-bearing visual signal for every task row in the
//! Dashboard and every state indicator in TaskDetail. Per DESIGN.md §2
//! the pill is `rounded-full px-2.5 py-0.5 text-xs font-medium text-white`
//! plus a single `bg-{color}` class picked from a 9-state match.
//!
//! ## Color palette (per DESIGN.md + acceptance criteria)
//!
//! | State         | Label        | Tailwind class  |
//! |---------------|--------------|-----------------|
//! | `Running`     | "Running"    | `bg-amber-500`  |
//! | `Idle`        | "Idle"       | `bg-slate-400`  |
//! | `Planned`     | "Planned"    | `bg-slate-400`  |
//! | `Implemented` | "Implemented"| `bg-slate-400`  |
//! | `Reviewed`    | "Reviewed"   | `bg-amber-500`  |
//! | `Done`        | "Done"       | `bg-emerald-500`|
//! | `Rejected`    | "Rejected"   | `bg-rose-500`   |
//! | `Failed`      | "Failed"     | `bg-rose-700`   |
//! | `Unknown`     | "Unknown"    | `bg-orange-500` |
//!
//! ## Accessibility
//!
//! Per DESIGN.md §6, `StatusPill` carries `role="status"` so screen
//! readers announce state changes when the pill is updated. The text
//! label is always present (color is never the only signal).
//!
//! ## Why a `match` (and not a lookup table)
//!
//! The exhaustive match forces a compiler error if `TaskState` ever
//! gains a new variant — better than a runtime miss. `TaskState` has 9
//! variants, and the match covers all 9.
use dioxus::prelude::*;
use crate::domain::TaskState;
#[component]
pub fn StatusPill(state: TaskState) -> Element {
    let (label, bg) = match state {
        TaskState::Running => ("Running", "bg-amber-500"),
        TaskState::Idle => ("Idle", "bg-slate-400"),
        TaskState::Planned => ("Planned", "bg-slate-400"),
        TaskState::Implemented => ("Implemented", "bg-slate-400"),
        TaskState::Reviewed => ("Reviewed", "bg-amber-500"),
        TaskState::Done => ("Done", "bg-emerald-500"),
        TaskState::Rejected => ("Rejected", "bg-rose-500"),
        TaskState::Failed => ("Failed", "bg-rose-700"),
        TaskState::Unknown => ("Unknown", "bg-orange-500"),
    };
    rsx! {
        span {
            class: "rounded-full px-2.5 py-0.5 text-xs font-medium text-white {bg}",
            role: "status",
            "{label}"
        }
    }
}
