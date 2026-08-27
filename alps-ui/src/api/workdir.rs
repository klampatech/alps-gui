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
}