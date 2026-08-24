//! UI-side mirror of the alps-core domain types.
//!
//! # Why this file exists
//!
//! SPEC §5 says we re-declare a thin `TaskId` wrapper here so the
//! Dioxus router can carry it as a typed path segment (`#[route("/tasks/:id")]
//! TaskDetail { id: TaskId }`). The router's derive needs the type to live
//! in the UI crate — that's the *only* intentional duplication between the
//! UI and the core crate. Everything else is re-exported from `alps_core::*`
//! and pulled through server functions at the boundary.
//!
//! # Conventions
//!
//! - `pub use` re-exports keep the canonical type in `alps_core` so the
//!   wire formats and on-disk JSON shapes match exactly.
//! - The UI-side `TaskId(String)` newtype is the one permitted divergence.
//!   Conversions to/from `alps_core::domain::TaskId` are explicit so we
//!   know which side of the boundary we're on at every call site.
//!
//! # When this file grows
//!
//! Don't add request/response DTOs here — those live in `src/api/`. Only
//! domain-shaped types (read-side mirrors of `alps_core`) belong here.
//!
//! # Stub noise
//!
//! Each re-export block carries `#[allow(unused_imports)]` because this
//! file lands BEFORE its consumers (the router in US-003, the
//! `StatusPill` / `FindingCard` / `ReceiptCard` components in US-004,
//! the dashboard fixtures in US-005, and the read-side server functions
//! in US-006). Once each consumer lands, that consumer's `use`
//! statement turns the re-export from "unused" back to "live", at which
//! point the `#[allow]` can drop. Keep this single global annotation
//! — per-item `#[allow]` would clutter the re-export lists.

use std::str::FromStr;

// ----- Re-exports from alps_core::summary (read-side summary surface) -----
//
// `TaskSummary` is consumed by the Dashboard fixtures (US-005) and the
// list server function (US-006). `TaskList` / `TaskDetail` are consumed
// by the read-side server functions (US-006). `TaskState` is consumed by
// `StatusPill` (US-004). `TaskNotFound` is the 404 sentinel shape used
// by `task_get` (US-006).

#[allow(unused_imports)]
pub use alps_core::summary::{TaskDetail, TaskList, TaskNotFound, TaskState, TaskSummary};

// ----- Re-exports from alps_core::receipt (final-output types) -----
//
// `Receipts`, `Receipt`, `ImplementMetrics`, and `ReviewSummary` are
// referenced transitively inside `TaskDetail` (the `Option<Receipts>`
// field and the `Option<ImplementMetrics>` / `Option<ReviewSummary>`
// fields on `TaskSummary`). The UI rarely names these directly — the
// `ReceiptCard` component (US-004) will be the primary direct consumer.
// Re-exported here per SPEC §4.1's "import the typed surface" rule.

#[allow(unused_imports)]
pub use alps_core::receipt::{ImplementMetrics, Receipt, Receipts, ReviewSummary};

// ----- Re-exports from alps_core::domain (state-machine + artifact types) -----
//
// Note: `TaskId` is intentionally NOT re-exported — see the UI-side
// newtype below. Everything else passes through unchanged so the UI can
// dereference fields by name once `TaskDetail` is deserialized.

#[allow(unused_imports)]
pub use alps_core::domain::{
    Artifact, ArtifactKind, Assertion, DefinitionOfDone, Feedback, Finding, Implementation, Plan,
    PlanId, Prompt, ReceiptId, Review, Severity, StoryId, UserStory,
};

// ----- UI-side TaskId newtype -------------------------------------------
//
// The Dioxus `Routable` derive needs the type in the UI crate (the macro
// generates trait impls that reference this crate's path, not alps-core's).
// We keep the inner representation identical (a `String` shaped like
// `YYYY-MM-DDTHHMMSS-<uuid8>`) so the two types are interchangeable at
// the string layer.

/// UI-side mirror of `alps_core::domain::TaskId`.
///
/// Wired into the Dioxus route enum as a typed path segment
/// (`#[route("/tasks/:id")] TaskDetail { id: TaskId }`). The router
/// derive requires the type to live in the UI crate — the macro emits
/// impls keyed to this crate's path, not `alps_core`'s. Conversions
/// to and from the canonical type are explicit (see the `From` impl
/// below) so the boundary is visible at every call site.
//
// US-003's Route enum references this struct in `TaskDetail { id: TaskId }`,
// `TaskLog { id: TaskId }`, `TaskDiff { id: TaskId }`. The router derive
// requires the type to derive `Clone + PartialEq + Eq + Hash + Debug` (✓)
// and to implement `FromStr + Default` for path-segment parsing (auto-impls
// `FromRouteSegment` via the `T: FromStr + Default` blanket) plus `Display`
// for rendering the URL back to a string (see the impl below). All four
// Extras already came from US-002; `Display` was added in US-003.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId(pub String);

// The struct itself is "live" now (referenced from Route enum variants),
// but `as_str` / `new` are still only called by tests + future stories
// (US-005 fixtures, US-006 server-function responses). Without this
// `#[allow(dead_code)]` rustc flags "associated items are never used" on
// the impl block — the struct-level allow doesn't cascade. The allow drops
// when US-005 lands, since the fixtures will call `TaskId::new` and the
// Dashboard rendering will call `id.as_str()` in `truncate_excerpt`.
#[allow(dead_code)]
impl TaskId {
    /// Borrow the inner string slice, matching `alps_core::domain::TaskId::as_str()`.
    // US-003+ consumers will use this in router param sites (e.g. `id.as_str()`).
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Construct a `TaskId` from a raw string. Does NOT validate the
    /// `YYYY-MM-DDTHHMMSS-<uuid8>` shape — the router is a transport,
    /// not a validator; deeper validation happens when `task_get`
    /// resolves the ID on the server side.
    // US-005 fixtures (and US-006 server-function responses) construct
    // `TaskId`s via this helper.
    pub fn new(s: impl Into<String>) -> Self {
        TaskId(s.into())
    }
}

/// `Display` is required by `dioxus_router`'s `Routable` derive — when the
/// router renders a `Route::TaskDetail { id }` back to a URL, it calls
/// `id.to_string()` on each dynamic segment (see
/// `dioxus_router::macro::segment::Segment::write_segment()` →
/// `quote! { write!(f, "/{}", #ident.to_string()) }`). We delegate to the
/// inner string so the URL round-trips byte-for-byte with what the router
/// parsed out.
impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The router parses path segments via `FromStr`. For an opaque ID we
/// accept any well-formed string — the server function is the source of
/// truth for whether the ID corresponds to a real task. Pairing this
/// with `Default` is what `dioxus_router`'s `FromRouteSegment` blanket
/// impl requires for auto-derivation.
impl FromStr for TaskId {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(TaskId(s.to_string()))
    }
}

/// `Default` is required by `dioxus_router`'s `FromRouteSegment` blanket
/// impl (it auto-derives for any `T: FromStr + Default`).
impl Default for TaskId {
    fn default() -> Self {
        TaskId(String::new())
    }
}

/// Convert a core `TaskId` into the UI-side `TaskId`. The reverse
/// direction would require a validation step (the core `TaskId`
/// enforces the `YYYY-MM-DDTHHMMSS-<uuid8>` shape via `TaskId::new()`);
/// we keep that as a deliberate boundary rather than introducing a
/// fallible conversion at every router param site.
impl From<alps_core::domain::TaskId> for TaskId {
    // US-006 server-function responses convert core `TaskId`s into UI-side
    // `TaskId`s at the boundary; US-005 fixtures construct via `TaskId::new`.
    fn from(core: alps_core::domain::TaskId) -> Self {
        TaskId(core.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_id_from_core_roundtrip_string() {
        let core = alps_core::domain::TaskId("2026-08-23T192049-abcdef01".to_string());
        let ui: TaskId = core.clone().into();
        assert_eq!(ui.as_str(), core.as_str());
        assert_eq!(ui.0, "2026-08-23T192049-abcdef01");
    }

    #[test]
    fn task_id_fromstr_accepts_any_well_formed_string() {
        let parsed: TaskId = "anything-here".parse().expect("infallible parse");
        assert_eq!(parsed.as_str(), "anything-here");
    }

    #[test]
    fn task_id_default_is_empty_string() {
        let d: TaskId = TaskId::default();
        assert_eq!(d.as_str(), "");
    }

    /// `Display` is what `dioxus_router`'s `Routable` derive calls to render
    /// dynamic segments back into URLs (`write!(f, "/{}", id.to_string())`).
    /// Round-tripping the inner string through `to_string()` must produce a
    /// byte-identical result — otherwise the router would silently URL-encode
    /// a different shape than what it parsed.
    #[test]
    fn task_id_display_matches_inner_string() {
        let raw = "2026-08-23T192049-abcdef01";
        let id = TaskId::new(raw);
        assert_eq!(id.to_string(), raw);
        assert_eq!(format!("{}", id), raw);
        // The empty-default case must render to an empty string and not the
        // literal "TaskId(...)" Debug representation.
        assert_eq!(TaskId::default().to_string(), "");
    }
}
