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

#[cfg(test)]
mod tests {
    //! StatusPill rendering tests (US-005 acceptance criterion #4).
    //!
    //! Each of the 9 `TaskState` variants is rendered through the
    //! `StatusPill` component and asserted to contain the exact label
    //! string from DESIGN.md §2 / US-004's color table.
    //!
    //! Rendering uses `dioxus_ssr::render_element`, which is a
    //! transitive dependency of `dioxus-fullstack` and is added as a
    //! `dev-dependency` in `Cargo.toml` so the regular `cargo build`
    //! doesn't pull it into the wasm artifact.

    use super::StatusPill;
    use crate::domain::TaskState;
    use dioxus::prelude::*;

    /// Render one `<StatusPill state={...} />` to an HTML string.
    fn render(state: TaskState) -> String {
        dioxus_ssr::render_element(rsx! {
            StatusPill { state }
        })
    }

    #[test]
    fn running_pill_renders_running_label() {
        let html = render(TaskState::Running);
        assert!(
            html.contains("Running"),
            "Running pill should contain 'Running' label: {html}",
        );
        assert!(
            html.contains("bg-amber-500"),
            "Running pill should use bg-amber-500: {html}",
        );
    }

    #[test]
    fn idle_pill_renders_idle_label() {
        let html = render(TaskState::Idle);
        assert!(html.contains("Idle"), "Idle pill should contain 'Idle' label: {html}");
        assert!(
            html.contains("bg-slate-400"),
            "Idle pill should use bg-slate-400: {html}",
        );
    }

    #[test]
    fn planned_pill_renders_planned_label() {
        let html = render(TaskState::Planned);
        assert!(
            html.contains("Planned"),
            "Planned pill should contain 'Planned' label: {html}",
        );
        assert!(
            html.contains("bg-slate-400"),
            "Planned pill should use bg-slate-400: {html}",
        );
    }

    #[test]
    fn implemented_pill_renders_implemented_label() {
        let html = render(TaskState::Implemented);
        assert!(
            html.contains("Implemented"),
            "Implemented pill should contain 'Implemented' label: {html}",
        );
        assert!(
            html.contains("bg-slate-400"),
            "Implemented pill should use bg-slate-400: {html}",
        );
    }

    #[test]
    fn reviewed_pill_renders_reviewed_label() {
        let html = render(TaskState::Reviewed);
        assert!(
            html.contains("Reviewed"),
            "Reviewed pill should contain 'Reviewed' label: {html}",
        );
        assert!(
            html.contains("bg-amber-500"),
            "Reviewed pill should use bg-amber-500: {html}",
        );
    }

    #[test]
    fn done_pill_renders_done_label() {
        let html = render(TaskState::Done);
        assert!(html.contains("Done"), "Done pill should contain 'Done' label: {html}");
        assert!(
            html.contains("bg-emerald-500"),
            "Done pill should use bg-emerald-500: {html}",
        );
    }

    #[test]
    fn rejected_pill_renders_rejected_label() {
        let html = render(TaskState::Rejected);
        assert!(
            html.contains("Rejected"),
            "Rejected pill should contain 'Rejected' label: {html}",
        );
        assert!(
            html.contains("bg-rose-500"),
            "Rejected pill should use bg-rose-500: {html}",
        );
    }

    #[test]
    fn failed_pill_renders_failed_label() {
        let html = render(TaskState::Failed);
        assert!(html.contains("Failed"), "Failed pill should contain 'Failed' label: {html}");
        assert!(
            html.contains("bg-rose-700"),
            "Failed pill should use bg-rose-700: {html}",
        );
    }

    #[test]
    fn unknown_pill_renders_unknown_label() {
        // The 9th variant — verified here rather than in FIXTURES per
        // US-005 acceptance criterion #4.
        let html = render(TaskState::Unknown);
        assert!(
            html.contains("Unknown"),
            "Unknown pill should contain 'Unknown' label: {html}",
        );
        assert!(
            html.contains("bg-orange-500"),
            "Unknown pill should use bg-orange-500: {html}",
        );
    }

    #[test]
    fn every_pill_carries_role_status_for_screen_readers() {
        // DESIGN.md §6 accessibility — `role="status"` makes screen
        // readers announce state changes. Verify every variant carries it.
        let states = [
            TaskState::Running,
            TaskState::Idle,
            TaskState::Planned,
            TaskState::Implemented,
            TaskState::Reviewed,
            TaskState::Done,
            TaskState::Rejected,
            TaskState::Failed,
            TaskState::Unknown,
        ];
        for state in states {
            let html = render(state);
            assert!(
                html.contains(r#"role="status""#),
                "{:?} pill should carry role=\"status\" for screen-reader announcements: {}",
                state,
                html,
            );
        }
    }
}
