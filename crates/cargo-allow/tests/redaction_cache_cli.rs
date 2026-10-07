//! Identity-redaction versus the persistent scan cache (#1920).
//!
//! The leak matrix from the issue thread, reduced to its key cells: a warm
//! cache populated under one `CARGO_ALLOW_REDACT_IDENTITY` mode must never
//! replay findings produced under the other mode. Every invocation is a
//! separate process, so the env var is passed per command without races.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The source-derived token that must never reach a redacted artifact.
const SECRET_CONTAINER: &str = "secret_mod::secret_unsafe_fn";

const FIXTURE_SOURCE: &str = "pub mod secret_mod {\n    pub unsafe fn secret_unsafe_fn() {\n        let raw: *const u8 = std::ptr::null();\n        let _ = raw;\n    }\n}\n";

struct TempGuard(PathBuf);
impl Drop for TempGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn make_fixture(label: &str) -> Result<(TempGuard, PathBuf), String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "cargo-allow-redaction-cache-{label}-{}-{stamp}",
        std::process::id()
    ));
    let guard = TempGuard(root.clone());
    fs::create_dir_all(root.join("src")).map_err(|error| error.to_string())?;
    fs::create_dir_all(root.join("policy")).map_err(|error| error.to_string())?;
    fs::write(root.join("src/lib.rs"), FIXTURE_SOURCE).map_err(|error| error.to_string())?;
    fs::write(
        root.join("policy/allow.toml"),
        "schema_version = 1\n\n[workspace]\nignored = []\ngenerated = []\n",
    )
    .map_err(|error| error.to_string())?;
    for args in [
        vec!["init"],
        vec!["config", "user.email", "cargo-allow@example.invalid"],
        vec!["config", "user.name", "cargo-allow test"],
        vec!["add", "--all"],
        vec!["commit", "-m", "redaction cache fixture"],
    ] {
        let status = Command::new("git")
            .args(&args)
            .current_dir(&root)
            .status()
            .map_err(|error| error.to_string())?;
        if !status.success() {
            return Err(format!("git {:?} failed with {status}", args));
        }
    }
    Ok((guard, root))
}

/// One `check` invocation as its own process; `redacted` toggles the env var.
fn run_check(
    root: &Path,
    redacted: bool,
    format: &str,
    persistent_cache: &str,
) -> Result<(String, Value), String> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-allow"));
    command
        .args([
            "check",
            "--root",
            root.to_str()
                .ok_or_else(|| "root is not UTF-8".to_string())?,
            "--config",
            "policy/allow.toml",
            "--persistent-cache",
            persistent_cache,
            "--format",
            format,
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if redacted {
        command.env("CARGO_ALLOW_REDACT_IDENTITY", "1");
    }
    let output = command.output().map_err(|error| error.to_string())?;
    let stdout = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
    if output.status.success() {
        return Err("known unsafe finding unexpectedly passed".to_string());
    }
    let report = if format == "json" {
        serde_json::from_str(&stdout).map_err(|error| format!("report JSON: {error}: {stdout}"))?
    } else {
        Value::Null
    };
    Ok((stdout, report))
}

/// One `why` invocation; `why` re-derives findings instead of serving the
/// scan cache, so it must honor the redaction mode against any warm cache.
fn run_why(root: &Path, redacted: bool) -> Result<Value, String> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-allow"));
    command.args([
        "why",
        "--root",
        root.to_str()
            .ok_or_else(|| "root is not UTF-8".to_string())?,
        "--config",
        "policy/allow.toml",
        "--kind",
        "unsafe",
        "--path",
        "src/lib.rs",
        "--line",
        "2",
        "--format",
        "json",
    ]);
    if redacted {
        command.env("CARGO_ALLOW_REDACT_IDENTITY", "1");
    }
    let output = command.output().map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!("why unexpectedly failed: {}", output.status));
    }
    let stdout = String::from_utf8(output.stdout).map_err(|error| error.to_string())?;
    serde_json::from_str(&stdout).map_err(|error| format!("why JSON: {error}: {stdout}"))
}

fn unsafe_container(report: &Value) -> Result<Option<String>, String> {
    report
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| "report has no findings array".to_string())?
        .iter()
        .find(|finding| {
            finding.get("kind").and_then(Value::as_str) == Some("unsafe")
                && finding.get("path").and_then(Value::as_str) == Some("src/lib.rs")
        })
        .map(|finding| {
            finding
                .get("container")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .ok_or_else(|| "unsafe src/lib.rs finding missing from report".to_string())
}

#[test]
fn warm_unredacted_cache_with_redaction_on_replays_redacted() -> Result<(), String> {
    let (_guard, root) = make_fixture("leak-cell")?;

    // Cold scan without the flag: identity text lands in the report and the
    // unredacted cache entry.
    let (cold_stdout, cold_report) = run_check(&root, false, "json", "on")?;
    assert!(
        cold_stdout.contains(SECRET_CONTAINER),
        "cold unredacted scan must carry the source-derived container"
    );
    assert_eq!(
        unsafe_container(&cold_report)?,
        Some(SECRET_CONTAINER.into())
    );

    // Warm unredacted cache + redaction on (#1920 leak cell): the report must
    // not replay the cached identity text anywhere.
    let (warm_stdout, warm_report) = run_check(&root, true, "json", "on")?;
    assert_eq!(
        unsafe_container(&warm_report)?,
        None,
        "warm unredacted cache with CARGO_ALLOW_REDACT_IDENTITY=1 must replay redacted"
    );
    assert!(
        !warm_stdout.contains(SECRET_CONTAINER),
        "redacted warm report leaked the source-derived container: {warm_stdout}"
    );

    // Warm redacted cache + redaction off: the unredacted entry is still
    // served — the two modes coexist instead of overwriting each other.
    let (back_stdout, back_report) = run_check(&root, false, "json", "on")?;
    assert!(
        back_stdout.contains(SECRET_CONTAINER),
        "unredacted warm run must keep serving the unredacted entry"
    );
    assert_eq!(
        unsafe_container(&back_report)?,
        Some(SECRET_CONTAINER.into())
    );
    Ok(())
}

#[test]
fn redacted_cold_scan_then_unredacted_run_recovers_identity() -> Result<(), String> {
    let (_guard, root) = make_fixture("poison-cell")?;

    // Cold scan WITH the flag: the redacted cache entry is written.
    let (cold_stdout, cold_report) = run_check(&root, true, "json", "on")?;
    assert!(
        !cold_stdout.contains(SECRET_CONTAINER),
        "cold redacted scan must not carry the container: {cold_stdout}"
    );
    assert_eq!(unsafe_container(&cold_report)?, None);

    // Warm run WITHOUT the flag (#1920 poisoning cell): identity must come
    // back without deleting the cache.
    let (warm_stdout, warm_report) = run_check(&root, false, "json", "on")?;
    assert!(
        warm_stdout.contains(SECRET_CONTAINER),
        "identity must recover from a warm redacted cache without deleting it"
    );
    assert_eq!(
        unsafe_container(&warm_report)?,
        Some(SECRET_CONTAINER.into())
    );
    Ok(())
}

#[test]
fn redaction_matrix_key_cells_hold_across_surfaces() -> Result<(), String> {
    let (_guard, root) = make_fixture("matrix")?;

    // Establish the warm unredacted cache.
    run_check(&root, false, "json", "on")?;

    // Markdown surface, warm unredacted cache + flag on: the identity-free
    // rendering must not leak regardless of cache mode.
    let (markdown_stdout, _) = run_check(&root, true, "markdown", "on")?;
    assert!(
        !markdown_stdout.contains(SECRET_CONTAINER),
        "markdown rendering leaked the source-derived container"
    );

    // why (json) re-derives: redaction holds against the warm unredacted
    // cache.
    let why_redacted = run_why(&root, true)?;
    assert_eq!(
        why_redacted
            .pointer("/finding/identity/container")
            .and_then(Value::as_str),
        None,
        "why json must redact identity against a warm unredacted cache"
    );

    // Cache-off baseline: redaction on a fresh scan needs no cache to work.
    let (off_stdout, off_report) = run_check(&root, true, "json", "off")?;
    assert!(!off_stdout.contains(SECRET_CONTAINER));
    assert_eq!(unsafe_container(&off_report)?, None);

    // Populate redacted entries, then probe why without the flag against the
    // warm redacted cache: identity re-derives.
    run_check(&root, true, "json", "on")?;
    let why_unredacted = run_why(&root, false)?;
    assert_eq!(
        why_unredacted
            .pointer("/finding/identity/container")
            .and_then(Value::as_str),
        Some(SECRET_CONTAINER),
        "why json must recover identity against a warm redacted cache"
    );
    Ok(())
}
