//! `ReceiptCard` — the final `Receipts` summary for a Done task (DESIGN.md §4).
//!
//! Renders every field on `alps_core::receipt::Receipts`: the plan
//! identifier, the judge model, the implement metrics (stories / iters /
//! elapsed), the review summary (assertions / critical findings), the
//! plan summary text, and the judged-at timestamp.
//!
//! ## Card chrome
//!
//! Same `rounded-lg border border-slate-200 bg-white p-4 shadow-sm`
//! pattern. The metric grid uses `grid grid-cols-2 gap-3` so the
//! implementation metrics and review summary sit side-by-side at
//! ≥ 640px and stack at < 640px.
//!
//! ## Why `format!("{:?}", receipts.task_id.0)`
//!
//! `receipts.task_id` is `alps_core::domain::TaskId(pub String)`. The UI
//! has its own `TaskId` newtype in `crate::domain` (routed through the
//! URL segment path). For the receipt we just print the inner string
//! so the user sees the canonical id (and to avoid pulling core's
//! `TaskId` into the UI's rendering pipeline).
//!
//! ## Plan summary rendering
//!
//! `whitespace-pre-wrap` preserves newlines from the canonical plan
//! summary (`alps-core` joins multi-line plan summaries with `\n\n`).
use dioxus::prelude::*;
use crate::domain::Receipts;
#[component]
pub fn ReceiptCard(receipts: Receipts) -> Element {
    rsx! {
        div { class: "rounded-lg border border-slate-200 bg-white p-4 shadow-sm space-y-3",
            h4 { class: "text-base font-medium text-slate-800", "Receipt" }
            div { class: "grid grid-cols-2 gap-3 text-sm",
                div { class: "space-y-1",
                    div { class: "text-xs uppercase tracking-wide text-slate-500", "Task ID" }
                    div { class: "text-xs font-mono text-slate-700 truncate", "{receipts.task_id.0}" }
                }
                div { class: "space-y-1",
                    div { class: "text-xs uppercase tracking-wide text-slate-500", "Plan ID" }
                    div { class: "text-xs font-mono text-slate-700 truncate", "{receipts.plan_id.0}" }
                }
                div { class: "space-y-1",
                    div { class: "text-xs uppercase tracking-wide text-slate-500", "Judge model" }
                    div { class: "text-sm text-slate-700", "{receipts.judge_model}" }
                }
                div { class: "space-y-1",
                    div { class: "text-xs uppercase tracking-wide text-slate-500", "Judged at" }
                    div { class: "text-sm text-slate-700 font-mono", "{receipts.judged_at}" }
                }
                div { class: "space-y-1 col-span-2",
                    div { class: "text-xs uppercase tracking-wide text-slate-500", "Implement metrics" }
                    div { class: "text-sm text-slate-700",
                        "{receipts.implement_metrics.stories_passed}/{receipts.implement_metrics.stories_total} stories · "
                        "{receipts.implement_metrics.iterations} iterations · "
                        "{receipts.implement_metrics.elapsed_secs}s elapsed"
                    }
                }
                div { class: "space-y-1 col-span-2",
                    div { class: "text-xs uppercase tracking-wide text-slate-500", "Review summary" }
                    div { class: "text-sm text-slate-700",
                        "{receipts.review_summary.assertions_passed}/{receipts.review_summary.assertions_total} assertions passed · "
                        "{receipts.review_summary.findings_count} findings · "
                        "{receipts.review_summary.critical_findings} critical"
                    }
                }
            }
            div { class: "space-y-1",
                div { class: "text-xs uppercase tracking-wide text-slate-500", "Plan summary" }
                p { class: "text-sm text-slate-700 whitespace-pre-wrap", "{receipts.plan_summary}" }
            }
        }
    }
}
