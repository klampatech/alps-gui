//! `get_workdir` + `set_workdir` server fns (M4-proper).
//!
//! Server-side persistence of the active workdir path to
//! `$HOME/.alps-ui-config.json`. The path is the single source of
//! truth for the workdir after M4-proper — `state.rs::default_workdir()`
//! reads this file first (before `ALPS_UI_WORKDIR` env var, before the
//! `$HOME/Development/alps-runs` fallback).
//!
//! ## File format
//!
//! Pretty-printed JSON, single key:
//!
//! ```json
//! {
//!   "workdir": "/home/kyle/Development/alps-runs"
//! }
//! ```
//!
//! We write atomically via temp-file-then-rename, same pattern as
//! `api/run.rs::write_alps_pids_json` (M3c).
//!
//! ## Why server-side only
//!
//! Decision recorded 2026-08-26 per Kyle: "server makes sense then".
//! The workdir is a single-host filesystem concept (the alps-runs
//! directory). Browser-side persistence would create a "what if the
//! browser says X and the server says Y" reconciliation question that
//! has no upside for the alps-runs use case. Future multi-workdir
//! picker needs server-side as the source of truth anyway.
//!
//! ## Server-fn surface
//!
//! - `get_workdir() -> Result<String, ServerFnError>` — no args;
//!   reads `$HOME/.alps-ui-config.json` and returns the persisted
//!   workdir, or `Ok` with an empty string if no config exists yet
//!   (so the wasm stub gets a parseable value to seed the context).
//! - `set_workdir(path: String) -> Result<(), ServerFnError>` —
//!   writes the given path to `$HOME/.alps-ui-config.json`. Returns
//!   the empty unit on success; the wasm caller then calls
//!   `Workdir::set` on its context to update the in-memory state.

use std::path::PathBuf;

use dioxus_fullstack_core::ServerFnError;
use dioxus_fullstack_macro::server;

/// Server-side: read the persisted workdir path. Returns `Ok(None)` if
/// the config file doesn't exist or doesn't have a `workdir` field —
/// callers (typically `state.rs::default_workdir()`) should fall back
/// to env var + `$HOME/Development/alps-runs` in that case.
#[cfg(feature = "server")]
pub fn read_config_workdir() -> std::io::Result<Option<String>> {
    let Some(path) = config_path() else {
        return Ok(None);
    };
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(&path)?;
    let parsed: serde_json::Value = serde_json::from_str(&contents)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(parsed
        .get("workdir")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string()))
}

/// Server-side: write the persisted workdir path to
/// `$HOME/.alps-ui-config.json`. Atomic (temp-file + rename).
///
/// Returns the path written, useful for tests that want to verify
/// the file exists at the expected location.
#[cfg(feature = "server")]
pub fn write_config_workdir(workdir: &str) -> std::io::Result<PathBuf> {
    let path = config_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "$HOME not set; cannot determine config file location",
        )
    })?;
    let json = serde_json::json!({ "workdir": workdir });
    let pretty = serde_json::to_string_pretty(&json)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    // Atomic write: temp file in the same directory (so the rename is
    // on the same filesystem), then rename. If the rename fails after
    // the temp file is written, the temp file is left behind — the
    // next successful write will overwrite it.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, pretty.as_bytes())?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

/// Server-side: the path to `$HOME/.alps-ui-config.json`. Returns
/// `None` if `$HOME` isn't set (very rare in practice but possible
/// in some stripped CI environments).
#[cfg(feature = "server")]
pub fn config_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".alps-ui-config.json"))
}

/// Server-side: expand a leading `~` or `~/...` to `$HOME` or
/// `$HOME/...`. Used at every server-fn entry point that takes a
/// `workdir` arg, because the wasm client can't link `std::env::var`
/// and ships a literal `~/...` value as its fallback.
///
/// ## Why this lives in `api/workdir.rs` instead of a util module
///
/// The fix is load-bearing for any workdir-string entry point — every
/// server fn that shells out via `Command::new("alps")` or reads
/// `<workdir>/.alps-pids.json` must expand first. Putting it here
/// keeps the import surface small (just `use crate::api::workdir::expand_workdir`)
/// and co-locates with the other workdir-handling helpers.
///
/// ## Why server-side instead of wasm-side
///
/// The wasm build can't read `$HOME` from the env. It also can't read
/// `$HOME/.alps-ui-config.json` (server-only file). The wasm's
/// `default_workdir()` therefore ships a literal `~/...` value, and
/// the server MUST be the side that resolves it to the user's actual
/// `$HOME/...`. Putting the expansion in the server is the only place
/// that has access to `$HOME`.
///
/// ## Behavior
///
/// - `"~/alps-runs"` → `"$HOME/alps-runs"` (with `$HOME` substituted)
/// - `"~"` → `"$HOME"`
/// - `"/abs/path"` → `"/abs/path"` (unchanged)
/// - `"relative/path"` → `"relative/path"` (unchanged — only the
///   leading `~` is special; this matches POSIX shell semantics)
/// - `""` → `""` (empty string passes through unchanged so callers
///   that want to skip expansion can pass an empty sentinel)
///
/// If `$HOME` isn't set (extremely rare), falls back to leaving the
/// `~` in place — `alps list --workdir ~/.alps-runs` would then
/// return the same error it always did, which is the safest fallback
/// (no silent wrong path).
#[cfg(feature = "server")]
pub fn expand_workdir(workdir: &str) -> String {
    if !workdir.starts_with('~') {
        return workdir.to_string();
    }
    let Some(home) = std::env::var("HOME").ok().filter(|h| !h.is_empty()) else {
        // No $HOME — leave the tilde in place. The downstream `alps`
        // invocation will fail the same way it always did, which is
        // better than silently picking the wrong path.
        return workdir.to_string();
    };
    if workdir == "~" {
        return home;
    }
    if let Some(rest) = workdir.strip_prefix("~/") {
        // `~/foo/bar` → `$HOME/foo/bar`. We don't use `Path::join` here
        // because `$HOME` might not end with a separator and we want
        // exactly one separator between home and the rest.
        let mut out = home;
        if !out.ends_with('/') {
            out.push('/');
        }
        out.push_str(rest);
        out
    } else {
        // `~user/foo` — we don't support user-specific expansion (no
        // passwd lookup). Pass through unchanged.
        workdir.to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────
// `#[server]` server fns — the public surface that the Settings page
// (and the App-mount init path) call.
// ─────────────────────────────────────────────────────────────────────

/// Server fn: read the persisted workdir. Returns `Ok("")` when no
/// config file exists yet (first run). The wasm client uses the
/// empty-string return to mean "no persisted choice — fall back to
/// `default_workdir()` on the client side".
#[server]
pub async fn get_workdir() -> Result<String, ServerFnError> {
    match read_config_workdir() {
        Ok(Some(path)) => Ok(path),
        Ok(None) => Ok(String::new()),
        Err(e) => Err(ServerFnError::ServerError {
            message: format!("read .alps-ui-config.json: {e}"),
            code: 500,
            details: None,
        }),
    }
}

/// Server fn: persist the workdir path. Atomic file write.
/// On error (e.g. $HOME not set, permission denied), returns the
/// `ServerFnError` so the Settings page can show the failure toast.
#[server]
pub async fn set_workdir(path: String) -> Result<(), ServerFnError> {
    write_config_workdir(&path)
        .map(|_| ())
        .map_err(|e| ServerFnError::ServerError {
            message: format!("write .alps-ui-config.json: {e}"),
            code: 500,
            details: None,
        })
}

#[cfg(test)]
#[cfg(feature = "server")]
mod tests {
    use super::*;

    /// When the config file doesn't exist, `read_config_workdir`
    /// returns `Ok(None)` — caller falls back to env var / $HOME.
    #[test]
    fn read_config_returns_none_when_file_missing() {
        // With no config file at $HOME/.alps-ui-config.json,
        // `read_config_workdir` returns Ok(None). This is the "fresh
        // install" case the Settings Save button's "not persisted
        // yet" toast references.
        let home = std::env::var("HOME").unwrap_or_default();
        let cfg_path = std::path::PathBuf::from(&home).join(".alps-ui-config.json");
        if !cfg_path.exists() {
            let result = read_config_workdir().unwrap();
            assert!(result.is_none(), "expected None for missing config file");
        }
    }

    /// Write + read serde shape: writing a path then reading should
    /// return the same path (we don't point $HOME at a tempdir
    /// because that's too invasive for a unit test, but we DO test
    /// the serde shape inline).
    #[test]
    fn write_then_read_serde_shape_roundtrips() {
        let json = serde_json::json!({ "workdir": "/tmp/test" });
        let s = serde_json::to_string(&json).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(
            parsed.get("workdir").and_then(|v| v.as_str()),
            Some("/tmp/test")
        );
    }

    /// `~/alps-runs` expands to `$HOME/alps-runs` with the user's
    /// actual home dir substituted. This is the load-bearing case
    /// for the wasm-side `default_workdir()` literal — without
    /// expansion, every cold-paint of the Dashboard sends `~/...`
    /// to the server which returns empty results.
    #[test]
    fn expand_workdir_tilde_slash() {
        let home = std::env::var("HOME").unwrap_or_default();
        assert_eq!(expand_workdir("~/alps-runs"), format!("{home}/alps-runs"));
        assert_eq!(expand_workdir("~/Development/alps-runs"), format!("{home}/Development/alps-runs"));
        assert_eq!(expand_workdir("~/foo/bar"), format!("{home}/foo/bar"));
    }

    /// `~` alone expands to just `$HOME`.
    #[test]
    fn expand_workdir_bare_tilde() {
        let home = std::env::var("HOME").unwrap_or_default();
        assert_eq!(expand_workdir("~"), home);
    }

    /// Absolute paths are returned unchanged. The expansion is only
    /// for the leading `~` — POSIX shell semantics.
    #[test]
    fn expand_workdir_absolute_path_unchanged() {
        assert_eq!(expand_workdir("/home/kyle/Development/alps-runs"), "/home/kyle/Development/alps-runs");
        assert_eq!(expand_workdir("/tmp/foo"), "/tmp/foo");
    }

    /// Relative paths without `~` are unchanged.
    #[test]
    fn expand_workdir_relative_unchanged() {
        assert_eq!(expand_workdir("./alps-runs"), "./alps-runs");
        assert_eq!(expand_workdir("alps-runs"), "alps-runs");
    }

    /// `~user/foo` (user-specific expansion, which requires passwd
    /// lookup) is NOT supported — passes through unchanged. Avoids
    /// silently picking the wrong path.
    #[test]
    fn expand_workdir_user_tilde_passthrough() {
        assert_eq!(expand_workdir("~root/foo"), "~root/foo");
    }

    /// `$HOME` without trailing slash still gets exactly one separator
    /// between home and the rest. Guards against double slashes.
    #[test]
    fn expand_workdir_handles_home_without_trailing_slash() {
        // Temporarily override HOME to a value without trailing slash.
        let saved = std::env::var("HOME").ok();
        std::env::set_var("HOME", "/home/kyle");
        let result = expand_workdir("~/alps-runs");
        assert_eq!(result, "/home/kyle/alps-runs");
        // Restore.
        match saved {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }

    /// Empty string passes through — callers may use `""` as a sentinel
    /// to skip expansion (e.g. when the workdir isn't required for a
    /// particular code path).
    #[test]
    fn expand_workdir_empty_passes_through() {
        assert_eq!(expand_workdir(""), "");
    }
}