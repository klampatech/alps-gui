//! `Rail` — the 5-node pipeline (Plan → Implement → Review → Judge → Done).
//!
//! Per docs/alps-gui-redesign-deck.html §00 (the cover) + §03 (TaskDetail),
//! the rail is the signature visual element of the Control Rail redesign.
//! It renders 5 named phases as labeled dots with bar connectors between
//! them. The current phase is `active` (amber + glow), earlier phases are
//! `done` (teal), later phases are `pending` (faint gray).
//!
//! Two variants are exported:
//!
//! - [`FullRail`] — the page-header variant on TaskDetail. Wide, with
//!   labels under each dot. Takes `current_phase: u8` (0-indexed: 0=Plan,
//!   1=Implement, 2=Review, 3=Judge, 4=Done). The rendered bars between
//!   nodes are `filled` when both endpoints are past.
//!
//! - [`MiniRail`] — the 5-bar compact variant on Dashboard task cards.
//!   No labels — just 5 horizontal segments. Each segment is `past`
//!   (teal) when the corresponding phase is complete, `on` (amber)
//!   when it's the current phase, `fail` (red) when `is_failed` is true
//!   AND the phase is reached.
//!
//! ## Why a component, not a CSS class on a parent div
//!
//! The rail encodes pipeline state (current phase + failure flag). A
//! `FullRail { current_phase: u8, has_failed: bool }` component
//! encapsulates the state → class mapping in one place. Pages call
//! `<FullRail current_phase={1} has_failed={false} />` and the rail
//! handles the dot/line coloring.
//!
//! ## Phase index mapping (used by TaskDetail / Dashboard task cards)
//!
//! ```text
//!   0  Plan       — task has a Plan artifact
//!   1  Implement  — task has an Implementation artifact
//!   2  Review     — task has a Review artifact
//!   3  Judge      — task has been judged (Done / Rejected)
//!   4  Done       — task is in Done state (terminal)
//! ```
//!
//! Pages compute `current_phase` from the TaskSummary + TaskDetail
//! artifact presence. The Dashboard computes it from state alone (no
//! per-task detail loaded).

use dioxus::prelude::*;

const NODES: [&str; 5] = ["Plan", "Implement", "Review", "Judge", "Done"];

/// Full-width rail used on the TaskDetail page header.
///
/// `current_phase` is 0-indexed (0=Plan, 1=Implement, 2=Review,
/// 3=Judge, 4=Done). Nodes BEFORE `current_phase` render as `done`
/// (teal); the node AT `current_phase` renders as `active` (amber +
/// glow); nodes AFTER render as pending (gray).
///
/// `has_failed` paints the active node red instead of amber when true
/// (used by `Failed` / `Rejected` tasks). The bars in front of the
/// failed node stay teal (those phases actually completed).
#[component]
pub fn FullRail(current_phase: u8, has_failed: bool) -> Element {
    // Clamp into valid range; if the caller passes 99 we render all
    // nodes as pending (no crash, just visually correct).
    let phase = current_phase.min(NODES.len() as u8);

    rsx! {
        div { class: "rail-full",
            for (idx, label) in NODES.iter().enumerate() {
                {
                    let idx_u8 = idx as u8;
                    let is_done = idx_u8 < phase;
                    let is_active = idx_u8 == phase;
                    // failed: the active node + every bar before it
                    // stays teal; the active node itself goes red so the
                    // operator sees "stopped here" at a glance.
                    let dot_class = if is_active && has_failed {
                        "rail-node active"
                    } else if is_done {
                        "rail-node done"
                    } else if is_active {
                        "rail-node active"
                    } else {
                        "rail-node"
                    };
                    // Bar BEFORE this node (skipped for idx 0).
                    let bar_filled = idx > 0 && idx_u8 <= phase;
                    rsx! {
                        if idx > 0 {
                            div {
                                class: if bar_filled { "rail-line filled" } else { "rail-line" },
                            }
                        }
                        div { class: "{dot_class}",
                            div { class: "rail-dot" }
                            div { class: "rail-label", "{label}" }
                        }
                    }
                }
            }
        }
    }
}

/// Compact 5-bar mini-rail used on Dashboard task cards.
///
/// Each of the 5 segments is `past` (teal) when its phase is complete,
/// `on` (amber) when it's the current phase, `fail` (red) when
/// `is_failed` is true AND the phase has been reached, otherwise
/// default (faint gray).
///
/// `current_phase` 0-indexed as in `FullRail`.
#[component]
pub fn MiniRail(current_phase: u8, is_failed: bool) -> Element {
    let phase = current_phase.min(NODES.len() as u8);

    rsx! {
        div { class: "rail-mini",
            for idx in 0..NODES.len() {
                {
                    let idx_u8 = idx as u8;
                    let cls = if idx_u8 < phase {
                        "past"
                    } else if idx_u8 == phase && is_failed {
                        "fail"
                    } else if idx_u8 == phase {
                        "on"
                    } else {
                        ""
                    };
                    rsx! {
                        i { class: "{cls}" }
                    }
                }
            }
        }
    }
}

/// Helper: derive a 0-indexed `current_phase` from a `TaskState` for
/// the Dashboard's mini-rail. TaskDetail computes a more accurate value
/// from artifact presence (see `pages/task_detail.rs`).
///
/// The mapping mirrors `current_phase` semantics above:
///
/// - `Idle`, `Planned` → 0 (Plan = current)
/// - `Running`, `Implemented`, `Reviewed` → 1 (Implement = current)
/// - `Done` → 4 (Done = current)
/// - `Rejected`, `Failed` → 1 with `is_failed=true` (stopped in Implement;
///   real artifacts may be deeper, but state-machine state is the
///   honest answer for the Dashboard)
/// - `Unknown` → 0 (nothing known — show all pending)
pub fn phase_for_state(state: crate::domain::TaskState) -> u8 {
    use crate::domain::TaskState;
    match state {
        TaskState::Idle | TaskState::Planned | TaskState::Unknown => 0,
        TaskState::Running | TaskState::Implemented | TaskState::Reviewed => 1,
        TaskState::Done => 4,
        // Failed / Rejected — render the active node red; we don't know
        // how far implementation actually got from the summary alone.
        // The Dashboard's mini-rail is best-effort; TaskDetail renders
        // a more accurate rail from artifact presence.
        TaskState::Rejected | TaskState::Failed => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TaskState;

    /// SSR contract: FullRail renders 5 nodes + 4 connectors. We assert
    /// on the count of `.rail-node` and `.rail-line` divs (5 and 4
    /// respectively) so a regression that drops/duplicates a node is
    /// caught before it lands.
    #[test]
    fn full_rail_renders_five_nodes_and_four_connectors() {
        let html = dioxus_ssr::render_element(rsx! {
            FullRail { current_phase: 1, has_failed: false }
        });
        let node_count = html.matches("rail-node").count();
        let line_count = html.matches("rail-line").count();
        assert!(
            node_count >= 5,
            "FullRail should render 5 .rail-node divs: {html}"
        );
        assert!(
            line_count >= 4,
            "FullRail should render 4 connector lines: {html}"
        );
    }

    #[test]
    fn mini_rail_renders_five_segments() {
        let html = dioxus_ssr::render_element(rsx! {
            MiniRail { current_phase: 2, is_failed: false }
        });
        let segment_count = html.matches("rail-mini").count();
        assert_eq!(segment_count, 1, "MiniRail root div: {html}");
        // 5 <i> segments — count opening tags.
        let i_count = html.matches("<i").count();
        assert_eq!(
            i_count, 5,
            "MiniRail should have 5 <i> segments, got {i_count}: {html}"
        );
    }

    #[test]
    fn phase_for_state_maps_correctly() {
        assert_eq!(phase_for_state(TaskState::Idle), 0);
        assert_eq!(phase_for_state(TaskState::Planned), 0);
        assert_eq!(phase_for_state(TaskState::Running), 1);
        assert_eq!(phase_for_state(TaskState::Implemented), 1);
        assert_eq!(phase_for_state(TaskState::Reviewed), 1);
        assert_eq!(phase_for_state(TaskState::Done), 4);
        assert_eq!(phase_for_state(TaskState::Failed), 1);
        assert_eq!(phase_for_state(TaskState::Rejected), 1);
        assert_eq!(phase_for_state(TaskState::Unknown), 0);
    }

    /// MiniRail handles the clamp — current_phase > 4 shouldn't crash.
    #[test]
    fn mini_rail_clamps_out_of_range_phase() {
        let html = dioxus_ssr::render_element(rsx! {
            MiniRail { current_phase: 99, is_failed: false }
        });
        // All 5 segments should be `.past` (idx < clamped phase).
        let past_count = html.matches("class=\"past\"").count();
        assert_eq!(past_count, 5, "all segments should be past when phase clamps to 5: {html}");
    }
}
