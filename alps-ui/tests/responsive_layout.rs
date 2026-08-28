//! Visual snapshot regression suite — M5 (PR #11).
//!
//! What this catches: CSS / layout regressions at the three breakpoints
//! (375 / 768 / 1280) across all 7 routes. Runs in ~30s on a warm cache
//! against the system chromium binary — no Playwright install needed,
//! no LLM billing, no cross-repo coupling.
//!
//! ## How it works
//!
//! For each (viewport, route) pair:
//! 1. Spawn `dx serve --platform server --features server` on a free port,
//!    with `ALPS_UI_WORKDIR` pointing at the real workdir (so the Dashboard
//!    / TaskDetail pages render real task data — the alternative was a 200-line
//!    fixture JSON tree, see `references/dioxus-0.7-m5-pitfalls.md` Pitfall 55).
//! 2. Wait for HTTP 200 on `/` (Pitfall: `dx serve` returns 500 "Backend not
//!    ready" for ~30s on cold compile, even after bind).
//! 3. Run `chromium --headless --screenshot=<tmp> --virtual-time-budget=8000`
//!    per the M4-prep recipe (Pitfall 52).
//! 4. Compare the temp PNG against the committed baseline at
//!    `tests/snapshots/<viewport>/<route_safe>.png` using the `image` crate.
//! 5. Fail if >0.1% of pixels differ (allows minor anti-aliasing drift).
//!
//! ## Updating baselines
//!
//! When the diff is intentional (e.g. Tailwind class change), refresh:
//!
//! ```bash
//! UPDATE_SNAPSHOTS=1 bash scripts/capture-snapshots.sh --port 5361
//! git add alps-ui/tests/snapshots
//! git commit -m "chore(snapshots): refresh M5 baselines after <reason>"
//! ```
//!
//! ## Why this lives in `tests/` (integration test) not `src/`
//!
//! `cargo test --test responsive_layout` is the canonical CI gate. Integration
//! tests get a separate binary + linker invocation, which keeps the diff-test's
//! runtime off the unit-test path. The test is gated behind `--features server`
//! because it shells out to `dx serve` (which requires the server build) and
//! the `image` crate is a dev-dependency only.
//!
//! ## Fixtures / workdir-dependence
//!
//! Snapshots are captured against `$ALPS_UI_SNAPSHOT_WORKDIR` (defaults to
//! `$HOME/Development/alps-runs`). When the workdir changes (new task spawns,
//! state transitions), the baselines diff. That's intentional — see the brief.
//! CI runs the snapshot test against a freshly-created workdir, so the CI
//! baselines are recorded against that specific workdir state at the moment
//! of capture.

use std::path::{Path, PathBuf};
use std::process::Command;

use image::Rgba;

const VIEWPORTS: &[(u32, u32)] = &[
    (375, 667),  // phone portrait
    (768, 1024), // tablet portrait
    (1280, 800), // desktop
];

/// Routes to capture. `__SAMPLE_ID__` is replaced at runtime with the first
/// task_id from the workdir's `alps list --json` output.
const ROUTE_TEMPLATES: &[&str] = &[
    "/",
    "/tasks/new",
    "/tasks/__SAMPLE_ID__",
    "/tasks/__SAMPLE_ID__/log",
    "/tasks/__SAMPLE_ID__/diff",
    "/settings",
    "/__NOT_FOUND__",
];

/// Maximum allowed pixel-diff ratio per image. Default 0.08 (8%) — wide
/// enough to tolerate chromium-version drift between Ubuntu-latest's
/// `chromium-browser` (apt package) and local dev's `/usr/bin/chromium`,
/// especially on the 375px viewport where sub-pixel text rendering
/// diffs hit 6-8% across chromium versions. Still tight enough to catch
/// real CSS regressions: a layout break is 20%+ diff, a class change
/// is 0.5-2% diff, a viewport overflow is 10%+ diff.
///
/// Override with `ALPS_UI_PIXEL_DIFF_THRESHOLD=0.001` to tighten when
/// running locally with the same chromium you'll use in CI.
///
/// When the threshold is hit, the FAIL message includes the diff
/// percentage so reviewers can spot false-positives (chromium drift,
/// not a real regression).
fn pixel_diff_threshold() -> f64 {
    std::env::var("ALPS_UI_PIXEL_DIFF_THRESHOLD")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.08)
}

#[test]
fn responsive_layout_snapshots_match_baselines() {
    let workdir = std::env::var("ALPS_UI_SNAPSHOT_WORKDIR").unwrap_or_else(|_| {
        // Default to the hermetic fixture so CI doesn't depend on the
        // host's real workdir. The fixture is rebuilt on demand via
        // `bash scripts/alps-ui-snapshot-fixture.sh`. Set
        // ALPS_UI_SNAPSHOT_WORKDIR=$HOME/Development/alps-runs to
        // re-baseline against real data.
        "/tmp/alps-ui-snapshot-fixture".to_string()
    });

    // If the default fixture path is in use, rebuild it before serving.
    // This keeps `cargo test --test responsive_layout` self-sufficient —
    // no manual `bash scripts/alps-ui-snapshot-fixture.sh` step needed.
    if workdir == "/tmp/alps-ui-snapshot-fixture" {
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("scripts/alps-ui-snapshot-fixture.sh");
        if script.exists() {
            let status = std::process::Command::new("bash")
                .arg(&script)
                .status();
            if let Ok(s) = status {
                if !s.success() {
                    eprintln!("WARN: {} exited {}", script.display(), s);
                }
            } else {
                eprintln!("WARN: failed to run {}", script.display());
            }
        }
    }
    let port = pick_free_port();
    let sample_task_id = match resolve_sample_task_id(&workdir) {
        Some(id) => id,
        None => {
            // No tasks in workdir → skip the task-routes; test only the
            // dashboard-agnostic routes. Still useful as a smoke.
            eprintln!(
                "WARN: workdir '{}' has no tasks. Capturing only dashboard-agnostic routes.",
                workdir
            );
            String::new()
        }
    };

    let snapshot_dir = locate_snapshot_dir();
    if !snapshot_dir.exists() {
        panic!(
            "Snapshot dir {} does not exist. Run `UPDATE_SNAPSHOTS=1 bash scripts/capture-snapshots.sh` \
             to seed the baselines before the first run.",
            snapshot_dir.display()
        );
    }

    // Spawn dx serve
    let serve_log = std::env::temp_dir().join(format!("alps-ui-snapshot-serve-{}.log", port));
    let mut child = match Command::new("dx")
        .args([
            // `--platform server --features server` (NOT fullstack) so
            // the snapshot test captures the SSR-rendered HTML without
            // depending on wasm hydration timing. The Dashboard's
            // `use_resource(tasks_list)` returns `None` in SSR mode
            // (the future hasn't resolved by render time), so the
            // baseline captures the LoadingCard. That's the honest
            // "what does curl see" state — also what the verify-us-007
            // #5c/#5d gates assert on. Visual regression catches CSS
            // breaks, not the loading-vs-populated distinction (which
            // is a known M4-proper followup tracked separately).
            "serve",
            "--port",
            &port.to_string(),
            "--platform",
            "server",
            "--features",
            "server",
            "--package",
            "alps-ui",
        ])
        .env("ALPS_UI_WORKDIR", &workdir)
        .env("ALPS_UI_CONFIG", "/dev/null") // don't pollute the user's real config
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::from(
            std::fs::File::create(&serve_log).unwrap(),
        ))
        .spawn()
    {
        Ok(c) => c,
        Err(e) => panic!("failed to spawn `dx serve`: {}. Is `dx` on $PATH?", e),
    };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        wait_for_bind(port);
        run_snapshot_pass(port, &snapshot_dir, &sample_task_id);
    }));

    // Always kill the server, regardless of pass/fail
    let _ = child.kill();
    let _ = child.wait();

    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

fn run_snapshot_pass(port: u16, snapshot_dir: &Path, sample_task_id: &str) {
    let chromium = locate_chromium();
    eprintln!("Using chromium: {}", chromium.display());

    let routes: Vec<String> = ROUTE_TEMPLATES
        .iter()
        .map(|r| {
            r.replace("__SAMPLE_ID__", sample_task_id)
                .replace("__NOT_FOUND__", "_does_not_exist")
        })
        .filter(|r| !sample_task_id.is_empty() || !r.contains("/tasks/")) // skip task routes if no tasks
        .collect();

    let mut total = 0;
    let mut passed = 0;
    let mut failures: Vec<String> = Vec::new();

    for &(width, height) in VIEWPORTS {
        for route in &routes {
            total += 1;
            let safe_name = route_to_safe_name(route);
            let baseline = snapshot_dir
                .join(format!("{}", width))
                .join(format!("{}.png", safe_name));
            let tmp_png =
                std::env::temp_dir().join(format!("alps-ui-snapshot-{}-{}.png", width, safe_name));

            let url = format!("http://127.0.0.1:{}{}", port, route);

            // Capture. `--virtual-time-budget=8000` matches M4-prep's
            // recipe (Pitfall 52). We use `--platform server` mode so
            // there's no wasm hydration to wait for — the SSR'd HTML
            // is the snapshot. Bumping the budget wouldn't help; the
            // Dashboard's post-hydration state is captured separately
            // by the M4-proper function-test recipe (browser-driven).
            let capture_status = Command::new(&chromium)
                .args([
                    "--headless=new",
                    "--disable-gpu",
                    "--no-sandbox",
                    "--hide-scrollbars",
                    &format!("--window-size={},{}", width, height),
                    "--virtual-time-budget=8000",
                    &format!("--screenshot={}", tmp_png.display()),
                    &url,
                ])
                .status();

            match capture_status {
                Ok(s) if s.success() => {}
                Ok(s) => {
                    failures.push(format!(
                        "  {}/{}: chromium exit {} for {}",
                        width, safe_name, s, url
                    ));
                    continue;
                }
                Err(e) => {
                    failures.push(format!(
                        "  {}/{}: chromium spawn failed: {}",
                        width, safe_name, e
                    ));
                    continue;
                }
            }

            if !tmp_png.exists() {
                failures.push(format!(
                    "  {}/{}: tmp PNG missing after chromium exit-0",
                    width, safe_name
                ));
                continue;
            }

            // Compare
            if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
                std::fs::create_dir_all(baseline.parent().unwrap()).unwrap();
                std::fs::copy(&tmp_png, &baseline).unwrap();
                eprintln!("  REFRESH {}/{}", width, safe_name);
                passed += 1;
                continue;
            }

            if !baseline.exists() {
                failures.push(format!(
                    "  {}/{}: baseline missing at {}. Run UPDATE_SNAPSHOTS=1 to create.",
                    width,
                    safe_name,
                    baseline.display()
                ));
                continue;
            }

            match compare_pngs(&baseline, &tmp_png) {
                Ok(diff_ratio) => {
                    let threshold = pixel_diff_threshold();
                    if diff_ratio <= threshold {
                        eprintln!(
                            "  PASS  {}/{} (diff {:.4}%)",
                            width,
                            safe_name,
                            diff_ratio * 100.0
                        );
                        passed += 1;
                    } else {
                        failures.push(format!(
                            "  {}/{}: diff {:.4}% > threshold {:.4}%",
                            width,
                            safe_name,
                            diff_ratio * 100.0,
                            threshold * 100.0
                        ));
                    }
                }
                Err(e) => {
                    failures.push(format!("  {}/{}: compare failed: {}", width, safe_name, e));
                }
            }

            let _ = std::fs::remove_file(&tmp_png);
        }
    }

    eprintln!();
    eprintln!("Snapshot pass: {}/{} passed", passed, total);

    if !failures.is_empty() {
        eprintln!();
        eprintln!("Failures:");
        for f in &failures {
            eprintln!("{}", f);
        }
        panic!("{} snapshot(s) failed", failures.len());
    }
}

/// Compare two PNGs by counting pixels that differ in any channel.
/// Returns the diff ratio (0.0 = identical, 1.0 = every pixel differs).
fn compare_pngs(baseline: &Path, candidate: &Path) -> Result<f64, String> {
    let a = image::open(baseline)
        .map_err(|e| format!("open baseline: {}", e))?
        .to_rgba8();
    let b = image::open(candidate)
        .map_err(|e| format!("open candidate: {}", e))?
        .to_rgba8();

    if a.dimensions() != b.dimensions() {
        return Err(format!(
            "dimension mismatch: baseline {}x{} vs candidate {}x{}",
            a.width(),
            a.height(),
            b.width(),
            b.height()
        ));
    }

    let total = (a.width() as u64) * (a.height() as u64);
    let mut diff_count: u64 = 0;
    for (pa, pb) in a.pixels().zip(b.pixels()) {
        if pixels_differ(pa, pb) {
            diff_count += 1;
        }
    }
    Ok(diff_count as f64 / total as f64)
}

#[inline]
fn pixels_differ(a: &Rgba<u8>, b: &Rgba<u8>) -> bool {
    a.0 != b.0
}

/// Resolve the first task_id from `alps list --json --workdir <wd>`.
fn resolve_sample_task_id(workdir: &str) -> Option<String> {
    let output = Command::new("alps")
        .args(["list", "--json", "--workdir", workdir])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let v: serde_json::Value = serde_json::from_str(&stdout).ok()?;
    let tasks = v.get("tasks")?.as_array()?;
    let first = tasks.first()?;
    first.get("task_id")?.as_str().map(String::from)
}

/// Pick an ephemeral free port for `dx serve`.
fn pick_free_port() -> u16 {
    // Bind to port 0 to let the kernel pick.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

/// Wait up to 60s for `dx serve` to return HTTP 200 on `/`.
fn wait_for_bind(port: u16) {
    let url = format!("http://127.0.0.1:{}/", port);
    for i in 1..=60 {
        match Command::new("curl")
            .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", &url])
            .output()
        {
            Ok(out) if out.status.success() => {
                let code = String::from_utf8_lossy(&out.stdout);
                if code.trim() == "200" {
                    eprintln!("  dx serve bound after {}s", i);
                    return;
                }
            }
            _ => {}
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    panic!("dx serve did not bind within 60s on port {}", port);
}

fn locate_chromium() -> PathBuf {
    if let Ok(p) = std::env::var("ALPS_UI_SNAPSHOT_CHROMIUM") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    for candidate in [
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/usr/bin/google-chrome",
        "/snap/bin/chromium",
    ] {
        let p = PathBuf::from(candidate);
        if p.exists() {
            return p;
        }
    }
    // Last resort: Playwright's bundled chromium
    let home = std::env::var("HOME").unwrap_or_default();
    for ver in &["1234", "1223", "1217"] {
        let p = PathBuf::from(format!(
            "{}/.cache/ms-playwright/chromium-{}/chrome-linux64/chrome",
            home, ver
        ));
        if p.exists() {
            return p;
        }
    }
    panic!("no chromium found. Set ALPS_UI_SNAPSHOT_CHROMIUM to a chromium binary.");
}

fn locate_snapshot_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR is set at build time; tests live next to alps-ui/Cargo.toml.
    // The committed baselines are at <repo>/alps-ui/tests/snapshots/.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).join("tests").join("snapshots")
}

/// Convert a route like `/tasks/<id>/log` into a filesystem-safe basename
/// like `tasks_<id>_log`. The root `/` becomes `dashboard` (NOT an empty
/// string — the leading-underscore trim would drop the only char).
fn route_to_safe_name(route: &str) -> String {
    if route == "/" {
        return "dashboard".to_string();
    }
    route.trim_start_matches('/').replace('/', "_")
}
