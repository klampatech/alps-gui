//! Hardcoded `TaskSummary` fixtures for the Dashboard (US-005).
//!
//! Per US-005 + DESIGN.md §5, the Dashboard ships with a fixture list
//! instead of a live `use_resource(tasks_list)` call. The fixtures cover
//! EXACTLY 8 of the 9 `TaskState` variants: every state that appears in
//! normal operation gets one row.
//!
//! ## Why 8 fixtures (not 9)
//!
//! The 9th variant is `TaskState::Unknown`, which only appears when a
//! task directory exists but its `prompt.md` is missing — corruption,
//! mid-flight deletion, etc. It's NOT a normal state and shouldn't show
//! up in the Dashboard's default fixture list. Per US-005 acceptance
//! criterion #4, `Unknown` is verified separately through a unit test
//! on `StatusPill` rather than a fixture entry.
//!
//! ## Why `LazyLock<Vec<TaskSummary>>` (not `pub const FIXTURES: &[TaskSummary] = &[...]`)
//!
//! US-005's acceptance criterion describes a `pub const FIXTURES` array.
//! That syntax requires a `const` initializer, but `alps_core::summary::TaskSummary`
//! has four `String` fields and `String::from` / `to_string` / `format!` are
//! **not** `const fn` in stable Rust (heap allocation inside `const` is
//! unstable as `const_heap`). There is no `pub const FIXTURES: &[TaskSummary]`
//! that satisfies "8 distinct fixtures" without a runtime initializer.
//!
//! The smallest-impact alternative is `std::sync::LazyLock<Vec<TaskSummary>>`
//! (stable since 1.80). The static still appears as `FIXTURES` to callers,
//! and the data is built exactly once on first deref. Consumers that want a
//! `&[TaskSummary]` call `&*FIXTURES` or `FIXTURES.as_slice()`; the
//! reference they get is `&'static [TaskSummary]`. This preserves the
//! acceptance criterion's intent (8 fixtures covering all 8 normal states,
//! named `FIXTURES`, typed as `TaskSummary`) while working within stable
//! Rust's `const` rules. See `progress.txt`'s US-005 learnings for the
//! full escalation note.
//!
//! ## Why per-fixture `const_str_to_unix` calls
//!
//! `chrono::DateTime::from_timestamp` is `const fn`, so the UNIX-seconds
//! conversion is computed at compile time and baked into `.rodata` — the
//! only runtime work per fixture is the four `String::from` allocations
//! for `task_id` / `prompt_excerpt` / `judge_verdict` / `judge_model`,
//! which is bounded and one-time (LazyLock amortizes across the app's
//! lifetime).
use crate::domain::{TaskSummary, TaskState};
use std::sync::LazyLock;

/// One fixture per normal `TaskState` variant.
///
/// `Unknown` is intentionally absent — see the module-level docs and
/// the `fixtures_do_not_include_unknown` test.
///
/// Accessor pattern:
///
/// ```ignore
/// for t in FIXTURES.iter() { /* t: &TaskSummary */ }
///
/// // Or, to get `&'static [TaskSummary]`:
/// let slice: &'static [TaskSummary] = &*FIXTURES;
/// ```
pub static FIXTURES: LazyLock<Vec<TaskSummary>> = LazyLock::new(|| {
    vec![
        TaskSummary {
            task_id: String::from("2026-08-24T003012-1a2b3c4d"),
            state: TaskState::Idle,
            attempts: 0,
            prompt_excerpt: String::from(
                "Refactor the metrics aggregator to stream values incrementally instead of buffering in memory.",
            ),
            created_at: chrono::DateTime::from_timestamp(
                const_str_to_unix("2026-08-24T003012"),
                0,
            )
            .expect("fixture timestamp is valid"),
            completed_at: None,
            stories_passed: None,
            stories_total: None,
            iterations: None,
            elapsed_secs: None,
            review_assertions_passed: None,
            review_assertions_total: None,
            critical_findings: None,
            judge_verdict: None,
            judge_model: None,
        },
        TaskSummary {
            task_id: String::from("2026-08-24T001547-5e6f7a8b"),
            state: TaskState::Planned,
            attempts: 1,
            prompt_excerpt: String::from(
                "Add a settings page so users can change the workdir without restarting the app.",
            ),
            created_at: chrono::DateTime::from_timestamp(
                const_str_to_unix("2026-08-24T001547"),
                0,
            )
            .expect("fixture timestamp is valid"),
            completed_at: None,
            stories_passed: None,
            stories_total: None,
            iterations: None,
            elapsed_secs: None,
            review_assertions_passed: None,
            review_assertions_total: None,
            critical_findings: None,
            judge_verdict: None,
            judge_model: None,
        },
        TaskSummary {
            task_id: String::from("2026-08-23T225612-9c0d1e2f"),
            state: TaskState::Implemented,
            attempts: 1,
            prompt_excerpt: String::from(
                "Migrate the persistence layer from bincode to postcard so cross-version reads work.",
            ),
            created_at: chrono::DateTime::from_timestamp(
                const_str_to_unix("2026-08-23T225612"),
                0,
            )
            .expect("fixture timestamp is valid"),
            completed_at: None,
            stories_passed: None,
            stories_total: None,
            iterations: None,
            elapsed_secs: None,
            review_assertions_passed: None,
            review_assertions_total: None,
            critical_findings: None,
            judge_verdict: None,
            judge_model: None,
        },
        TaskSummary {
            task_id: String::from("2026-08-23T213304-3a4b5c6d"),
            state: TaskState::Reviewed,
            attempts: 1,
            prompt_excerpt: String::from(
                "Wire the cancellation token through every long-running tokio task so shutdown is clean.",
            ),
            created_at: chrono::DateTime::from_timestamp(
                const_str_to_unix("2026-08-23T213304"),
                0,
            )
            .expect("fixture timestamp is valid"),
            completed_at: None,
            stories_passed: None,
            stories_total: None,
            iterations: None,
            elapsed_secs: None,
            review_assertions_passed: None,
            review_assertions_total: None,
            critical_findings: None,
            judge_verdict: None,
            judge_model: None,
        },
        TaskSummary {
            task_id: String::from("2026-08-23T195821-7e8f9a0b"),
            state: TaskState::Running,
            attempts: 2,
            prompt_excerpt: String::from(
                "Render a responsive Kanban view in the Dashboard so users can drag tasks between columns.",
            ),
            created_at: chrono::DateTime::from_timestamp(
                const_str_to_unix("2026-08-23T195821"),
                0,
            )
            .expect("fixture timestamp is valid"),
            completed_at: None,
            stories_passed: None,
            stories_total: None,
            iterations: None,
            elapsed_secs: None,
            review_assertions_passed: None,
            review_assertions_total: None,
            critical_findings: None,
            judge_verdict: None,
            judge_model: None,
        },
        TaskSummary {
            task_id: String::from("2026-08-23T181045-1c2b3d4e"),
            state: TaskState::Done,
            attempts: 1,
            prompt_excerpt: String::from(
                "Add structured logging with tracing-subscriber so each task gets a request-scoped span.",
            ),
            created_at: chrono::DateTime::from_timestamp(
                const_str_to_unix("2026-08-23T181045"),
                0,
            )
            .expect("fixture timestamp is valid"),
            completed_at: chrono::DateTime::from_timestamp(
                const_str_to_unix("2026-08-23T182317"),
                0,
            ),
            stories_passed: Some(4),
            stories_total: Some(4),
            iterations: Some(3),
            elapsed_secs: Some(7320),
            review_assertions_passed: Some(7),
            review_assertions_total: Some(7),
            critical_findings: Some(0),
            judge_verdict: Some(String::from("ACCEPTED")),
            judge_model: Some(String::from("gpt-5-mini")),
        },
        TaskSummary {
            task_id: String::from("2026-08-23T160228-5a6b7c8d"),
            state: TaskState::Rejected,
            attempts: 1,
            prompt_excerpt: String::from(
                "Replace the bespoke retry policy with a backoff library; reset Rejected tasks on retry.",
            ),
            created_at: chrono::DateTime::from_timestamp(
                const_str_to_unix("2026-08-23T160228"),
                0,
            )
            .expect("fixture timestamp is valid"),
            completed_at: chrono::DateTime::from_timestamp(
                const_str_to_unix("2026-08-23T161902"),
                0,
            ),
            stories_passed: None,
            stories_total: None,
            iterations: Some(2),
            elapsed_secs: Some(5124),
            review_assertions_passed: Some(3),
            review_assertions_total: Some(6),
            critical_findings: Some(2),
            judge_verdict: Some(String::from("REJECTED")),
            judge_model: Some(String::from("gpt-5-mini")),
        },
        TaskSummary {
            task_id: String::from("2026-08-23T134517-9e0f1a2b"),
            state: TaskState::Failed,
            attempts: 1,
            prompt_excerpt: String::from(
                "Investigate the OOM crash when reading receipt bundles >500MB from cold storage.",
            ),
            created_at: chrono::DateTime::from_timestamp(
                const_str_to_unix("2026-08-23T134517"),
                0,
            )
            .expect("fixture timestamp is valid"),
            completed_at: chrono::DateTime::from_timestamp(
                const_str_to_unix("2026-08-23T135044"),
                0,
            ),
            stories_passed: None,
            stories_total: None,
            iterations: Some(1),
            elapsed_secs: Some(527),
            review_assertions_passed: None,
            review_assertions_total: None,
            critical_findings: None,
            judge_verdict: Some(String::from("FAILED")),
            judge_model: None,
        },
    ]
});

/// Compile-time UNIX-seconds converter for `YYYY-MM-DDTHHMMSS`.
///
/// Howard Hinnant's `days_from_civil` is the standard algorithm for
/// going from a Gregorian date to a UNIX day count without going
/// through a calendar library. The fixture timestamps are stable
/// strings, so we can do the conversion at compile time.
const fn const_str_to_unix(s: &str) -> i64 {
    let bytes = s.as_bytes();
    let y = digit_at(bytes, 0) * 1000 + digit_at(bytes, 1) * 100
        + digit_at(bytes, 2) * 10 + digit_at(bytes, 3);
    let m = digit_at(bytes, 5) * 10 + digit_at(bytes, 6);
    let d = digit_at(bytes, 8) * 10 + digit_at(bytes, 9);
    let h = digit_at(bytes, 11) * 10 + digit_at(bytes, 12);
    let mi = digit_at(bytes, 13) * 10 + digit_at(bytes, 14);
    let s = digit_at(bytes, 15) * 10 + digit_at(bytes, 16);
    days_from_civil(y, m as u32, d as u32) * 86_400
        + h * 3600 + mi * 60 + s
}

/// Howard Hinnant's `days_from_civil` — days since 1970-01-01 (UNIX epoch).
const fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + (doe as i64) - 719_468
}

const fn digit_at(bytes: &[u8], idx: usize) -> i64 {
    (bytes[idx] - b'0') as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixtures_cover_eight_distinct_states() {
        let mut states: Vec<TaskState> = FIXTURES.iter().map(|t| t.state).collect();
        states.sort_by_key(|s| format!("{:?}", s));
        assert_eq!(
            states,
            vec![
                TaskState::Done,
                TaskState::Failed,
                TaskState::Idle,
                TaskState::Implemented,
                TaskState::Planned,
                TaskState::Rejected,
                TaskState::Reviewed,
                TaskState::Running,
            ],
            "FIXTURES should contain one row for each of the 8 normal TaskState variants",
        );
    }

    #[test]
    fn fixtures_do_not_include_unknown() {
        assert!(
            FIXTURES.iter().all(|t| t.state != TaskState::Unknown),
            "Unknown is the corruption-only 9th variant; verify it via StatusPill tests instead",
        );
    }

    #[test]
    fn every_fixture_has_a_realistic_prompt_excerpt() {
        for t in FIXTURES.iter() {
            assert!(
                t.prompt_excerpt.len() >= 20,
                "fixture {:?} has a suspiciously short prompt_excerpt: {:?}",
                t.task_id,
                t.prompt_excerpt,
            );
            assert!(
                t.prompt_excerpt.len() <= 200,
                "fixture {:?} exceeds the 200-char TaskSummary cap: {} chars",
                t.task_id,
                t.prompt_excerpt.len(),
            );
        }
    }

    #[test]
    fn done_rejected_failed_carry_completed_at() {
        for t in FIXTURES.iter() {
            if matches!(
                t.state,
                TaskState::Done | TaskState::Rejected | TaskState::Failed
            ) {
                assert!(
                    t.completed_at.is_some(),
                    "{:?} should have completed_at set",
                    t.state,
                );
                assert!(
                    t.completed_at.unwrap() >= t.created_at,
                    "{:?}: completed_at must be at or after created_at",
                    t.state,
                );
            }
        }
    }

    #[test]
    fn done_has_full_metrics() {
        let done = FIXTURES
            .iter()
            .find(|t| t.state == TaskState::Done)
            .expect("FIXTURES contains a Done row");
        assert_eq!(done.stories_passed, Some(4));
        assert_eq!(done.stories_total, Some(4));
        assert!(done.judge_verdict.as_deref() == Some("ACCEPTED"));
        assert!(done.judge_model.is_some());
    }

    #[test]
    fn const_str_to_unix_matches_known_epoch_seconds() {
        // 1970-01-01T00:00:00 = 0.
        assert_eq!(const_str_to_unix("1970-01-01T000000"), 0);
        // 2000-01-01T00:00:00 = 946_684_800.
        assert_eq!(const_str_to_unix("2000-01-01T000000"), 946_684_800);
        // 2026-08-23T18:10:45Z — see https://www.epochconverter.com/.
        assert_eq!(const_str_to_unix("2026-08-23T181045"), 1_787_508_645);
    }
}
