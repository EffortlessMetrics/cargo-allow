use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[test]
fn campaign_skill_contract_is_wired() -> Result<(), Box<dyn Error>> {
    let root = repository_root()?;

    // Packaged cargo-allow tests run without the repository guidance tree.
    if !root.join(".git").exists() {
        return Ok(());
    }

    let skill = read(&root, ".agents/skills/cargo-allow-0.2-campaign/SKILL.md")?;
    require(
        skill.starts_with("---\nname: cargo-allow-0.2-campaign\n"),
        "cargo-allow-0.2-campaign skill must retain its canonical front matter",
    )?;
    for marker in [
        "description: Orchestrate and execute reversible implementation issues for the cargo-allow 0.2.0 campaign (#3768)",
        "## Trigger",
        "## Required Live Inputs",
        "## Session Lane Classification",
        "ReversibleImplementation",
        "ReadOnlyReview",
        "ExternalObservation",
        "RootDecision",
        "IrreversibleOperation",
        "BlockedOrStale",
        "## Issue Selection Algorithm",
        "## Implementation Packet",
        "## PR Lifecycle and Merge Rules",
        "## Release Immutability Law",
        "## Evidence and Post-Merge Reporting",
        "## Claim Boundary",
        "Hand substantive PR review to `review-current-head` after the author head is final",
        "Never delete, move, or recreate `v0.2.0-rc.1`",
        "Never treat a tag push or latest green CI run as release authorization",
        "hard STOP for separate explicit #3760 release authorization",
    ] {
        require_contains(&skill, marker, "canonical campaign skill")?;
    }

    let agents = read(&root, "AGENTS.md")?;
    require_contains(
        &agents,
        ".agents/skills/cargo-allow-0.2-campaign",
        "AGENTS.md",
    )?;

    let gemini = read(&root, "GEMINI.md")?;
    require_contains(&gemini, "cargo-allow-0.2-campaign", "GEMINI.md")?;

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
        &format!("{owner} is missing required campaign-contract marker: {needle}"),
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
