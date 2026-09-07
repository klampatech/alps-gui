//! `StatusPill` — color-coded badge for a `TaskState` (DESIGN.md §4).
//!
//! Control Rail redesign (deck §01): the pill is now a dim background +
//! colored text + leading dot, NOT the pre-redesign solid-color pill.
//!
//! ## Visual states (6 styles, per deck §01)
//!
//! | Label       | bg             | text    |
//! |-------------|----------------|---------|
//! | `Idle`      | `--gray-dim`   | `--gray`|
//! | `Planned`   | `--violet-dim` | `--violet`|
//! | `Running`   | `--amber-dim`  | `--amber` |
//! | `Done`      | `--teal-dim`   | `--teal` |
//! | `Failed`    | `--red-dim`    | `--red`  |
//! | `Rejected`  | transparent    | `--red` + 1px red border |
//!
//! ## State-to-style mapping (9-variant exhaustive match)
//!
//! `TaskState` has 9 variants. The pill collapses some of them to the
//! same visual style (e.g. `Idle` and `Unknown` both use the gray
//! `Idle` style; `Implemented` and `Reviewed` use the amber `Running`
//! style — the user can tell them apart by the `M3c` Plan/Review
//! accordions on TaskDetail). The match MUST stay exhaustive over
//! all 9 variants — `cargo test` enforces this.
//!
//! ## Accessibility
//!
//! Per DESIGN.md §6, `StatusPill` carries `role="status"` so screen
//! readers announce state changes when the pill is updated. The text
//! label is always present (color is never the only signal).
//!
//! ## Why a CSS class (`.pill-*`) instead of inline styles
//!
//! The 6 pill styles live in `assets/main.css` as `.pill-*` classes.
//! Class-based styling keeps the JSX clean and lets us share the
//! visual style across the Dashboard task cards, the TaskDetail
//! header pill, and any future surfaces (toast notifications,
//! settings status indicators). All 6 classes use the design deck's
//! color tokens (no per-instance hex codes).
use dioxus::prelude::*;
use crate::domain::TaskState;

#[component]
pub fn StatusPill(state: TaskState) -> Element {
    // 9-variant exhaustive match — `cargo test` enforces every variant
    // is handled. The deck collapses 9 states onto 6 visual styles;
    // `Implemented` and `Reviewed` collapse onto `Running` style
    // (amber — "still moving through the pipeline"); `Unknown` and
    // `Idle` collapse onto `Idle` style (gray — "nothing to show").
    let pill_class = match state {
        TaskState::Running => "pill pill-running",
        TaskState::Idle => "pill pill-idle",
        TaskState::Planned => "pill pill-planned",
        TaskState::Implemented => "pill pill-running",
        TaskState::Reviewed => "pill pill-running",
        TaskState::Done => "pill pill-done",
        TaskState::Rejected => "pill pill-rejected",
        TaskState::Failed => "pill pill-failed",
        TaskState::Unknown => "pill pill-idle",
    };
    let label = match state {
        TaskState::Running => "Running",
        TaskState::Idle => "Idle",
        TaskState::Planned => "Planned",
        TaskState::Implemented => "Implemented",
        TaskState::Reviewed => "Reviewed",
        TaskState::Done => "Done",
        TaskState::Rejected => "Rejected",
        TaskState::Failed => "Failed",
        TaskState::Unknown => "Unknown",
    };
    rsx! {
        span {
            class: "{pill_class}",
            role: "status",
            span { class: "dot" }
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
    //! string from the design deck + the right pill class.
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

    /// Every variant carries the matching label string.
    #[test]
    fn every_variant_renders_expected_label() {
        let cases: &[(TaskState, &str)] = &[
            (TaskState::Running, "Running"),
            (TaskState::Idle, "Idle"),
            (TaskState::Planned, "Planned"),
            (TaskState::Implemented, "Implemented"),
            (TaskState::Reviewed, "Reviewed"),
            (TaskState::Done, "Done"),
            (TaskState::Rejected, "Rejected"),
            (TaskState::Failed, "Failed"),
            (TaskState::Unknown, "Unknown"),
        ];
        for (state, expected) in cases {
            let html = render(state.clone());
            assert!(
                html.contains(expected),
                "{:?} pill should contain '{expected}' label: {html}",
                state,
            );
        }
    }

    /// Map each variant to the deck's pill class (6 styles; 9 variants
    /// collapse onto them). Replaces the pre-redesign
    /// `bg-{color}-{shade}` assertions.
    #[test]
    fn every_variant_uses_expected_pill_class() {
        let cases: &[(TaskState, &str)] = &[
            (TaskState::Running, "pill-running"),
            (TaskState::Idle, "pill-idle"),
            (TaskState::Planned, "pill-planned"),
            (TaskState::Implemented, "pill-running"),
            (TaskState::Reviewed, "pill-running"),
            (TaskState::Done, "pill-done"),
            (TaskState::Rejected, "pill-rejected"),
            (TaskState::Failed, "pill-failed"),
            (TaskState::Unknown, "pill-idle"),
        ];
        for (state, expected_class) in cases {
            let html = render(state.clone());
            assert!(
                html.contains(expected_class),
                "{:?} pill should carry class '{expected_class}': {html}",
                state,
            );
        }
    }

    /// Accessibility — every pill carries `role="status"` for screen
    /// readers. The label is always present so color isn't the only
    /// signal.
    #[test]
    fn every_pill_carries_role_status_for_screen_readers() {
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
                "{:?} pill should carry role=\"status\": {}",
                state,
                html,
            );
        }
    }

    /// Leading dot — the deck §01 design has a `6px × 6px` colored
    /// dot before the label. Assert each pill carries a `.dot` element.
    #[test]
    fn every_pill_has_leading_dot() {
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
                html.contains(r#"class="dot""#),
                "{:?} pill should carry a .dot element: {}",
                state,
                html,
            );
        }
    }
}
