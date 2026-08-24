use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[test]
fn rc1_incident_reconciliation_is_recorded() -> Result<(), Box<dyn Error>> {
    let root = repository_root()?;

    if !root.join(".git").exists() {
        return Ok(());
    }

    let release_doc = read(&root, "docs/release/0.2.0-rc.1.md")?;
    for marker in [
        "Public prerelease with incident lineage (`0.2.0-rc.1`)",
        "Final Candidate Eligibility**: `NotReusable`",
        "Rollback Baseline**: `0.1.11`",
        "Workflow Run `32698363934`",
        "cargo install cargo-allow --version 0.2.0-rc.1",
        "Tag `v0.2.0-rc.1` is immutable and will not be moved or republished",
    ] {
        require_contains(&release_doc, marker, "0.2.0-rc.1 release doc")?;
    }

    let incident_doc = read(
        &root,
        "docs/release/incidents/2026-08-24-rc1-tag-movement.md",
    )?;
    for marker in [
        "INCIDENT-2026-08-24-RC1-TAG-MOVEMENT",
        "CargoAllowRcPublicationIncidentV1",
        "PublicPrereleaseWithIncident",
        "PublishedAcrossCandidateHistory",
        "32684125678",
        "32689821900",
        "32698363934",
        "allow-core",
        "allow-diff",
        "cargo-allow",
        "Tag `v0.2.0-rc.1` must never be deleted, moved, retagged, or overwritten",
        "0.1.11",
    ] {
        require_contains(&incident_doc, marker, "rc.1 incident record")?;
    }

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
    require(
        normalized_haystack.contains(&normalized_needle),
        &format!("{owner} is missing required marker: {needle}"),
    )
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn require(condition: bool, message: &str) -> Result<(), Box<dyn Error>> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other(message.to_string()).into())
    }
}
