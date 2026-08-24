use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn upgrade_guide_contract_is_recorded() -> Result<(), Box<dyn Error>> {
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
        "ReadUnchanged",
        "ReadWithCompatibleProjection",
        "Pre-Upgrade State Capture",
        "Rollback Journey",
    ] {
        require_contains(&doc, marker, "upgrade-and-rollback guide")?;
    }

    Ok(())
}

#[test]
fn cli_diagnostics_and_compatibility_execution() -> Result<(), Box<dyn Error>> {
    let bin = env!("CARGO_BIN_EXE_cargo-allow");

    // Verify binary executes and outputs version
    let output = Command::new(bin).arg("--version").output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cargo-allow"));

    // Verify release-identity confirms stable 0.1.11 compatibility
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
