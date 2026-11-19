use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn upgrade_guide_preserves_baseline_and_limits_rollback_scope() -> Result<(), Box<dyn Error>> {
    let root = repository_root()?;
    if !root.join(".git").exists() {
        return Ok(());
    }

    let doc = read(&root, "docs/how-to/upgrade-and-rollback-0.1.11-to-0.2.0.md")?;
    for marker in [
        "HOWTO-UPGRADE-ROLLBACK-0.1.11-TO-0.2.0",
        "UpgradeRollbackGuideV1",
        "0.1.11",
        "0.2.0",
        "Before the final `0.2.0` release is published, keep shared CI pinned to `0.1.11`.",
        "not a compatibility guarantee",
        "Linux with Bash and GNU utilities",
        "Rust 1.85 or later for published",
        "Rust 1.95 or later for `0.2.0`",
        "Keep the copies and baseline evidence outside directories you might clean",
        "Do not remove the entire output directory.",
        "Restore an entire file only when it contains no unrelated changes",
        "Do not use the current `HEAD` as the pre-upgrade reference",
        r#"check_version "$baseline_bin" 0.1.11 &&"#,
        r#"$(mktemp -d "target/cargo-allow/$2.XXXXXXXX")"#,
        "sha256sum --check --strict",
        "git diff --cached -- .github/workflows/ci.yml",
        "If the CI pin and unrelated changes overlap, stop",
        "cargo install cargo-allow --version 0.2.0 --locked --registry crates-io",
        "Pre-Upgrade State Capture",
        "Rollback Journey",
    ] {
        require_contains(&doc, marker, "upgrade-and-rollback guide")?;
    }

    for unsafe_guidance in [
        "rm -rf target/cargo-allow",
        "git checkout HEAD -- .github/workflows/ci.yml",
        "ensure zero data loss",
    ] {
        if doc.contains(unsafe_guidance) {
            return Err(io::Error::other(format!(
                "upgrade-and-rollback guide contains unsafe or unproven guidance: {unsafe_guidance}"
            ))
            .into());
        }
    }

    Ok(())
}

#[test]
fn release_identity_parses_published_baseline_version() -> Result<(), Box<dyn Error>> {
    let bin = env!("CARGO_BIN_EXE_cargo-allow");

    // Verify binary executes and outputs version
    let output = Command::new(bin).arg("--version").output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cargo-allow"));

    // This checks identity parsing only, not compatibility between installed releases.
    let output = Command::new(bin)
        .args(["release-identity", "--version", "0.1.11"])
        .output()?;
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["version"], "0.1.11");
    assert_eq!(json["channel"], "stable");

    Ok(())
}

fn repository_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = manifest_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow manifest has no crates parent"))?;
    let root = crates_dir
        .parent()
        .ok_or_else(|| io::Error::other("cargo-allow crates directory has no repository parent"))?;
    Ok(root.to_path_buf())
}

fn read(root: &Path, relative: &str) -> Result<String, Box<dyn Error>> {
    let path = root.join(relative);
    fs::read_to_string(&path)
        .map(|text| text.replace("\r\n", "\n"))
        .map_err(|error| {
            io::Error::other(format!("failed to read {}: {error}", path.display())).into()
        })
}

fn require_contains(haystack: &str, needle: &str, owner: &str) -> Result<(), Box<dyn Error>> {
    let normalized_haystack = normalize_whitespace(haystack);
    let normalized_needle = normalize_whitespace(needle);
    if normalized_haystack.contains(&normalized_needle) {
        Ok(())
    } else {
        Err(io::Error::other(format!("{owner} is missing required marker: {needle}")).into())
    }
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
