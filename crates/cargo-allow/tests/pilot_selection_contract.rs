use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[test]
fn pilot_selection_contract_is_recorded() -> Result<(), Box<dyn Error>> {
    let root = repository_root()?;

    if !root.join(".git").exists() {
        return Ok(());
    }

    let doc = read(&root, "docs/pilots/0.2.0-rc.1-pilot-selection.md")?;
    for marker in [
        "PILOT-SELECTION-2026-08-24-0.2.0-RC1",
        "CargoAllowPilotSelectionV1",
        "#3771",
        "#2466",
        "#2467",
        "#3151",
        "cargo install cargo-allow --version 0.2.0-rc.1",
        "0.1.11",
        "copybook-rs",
        "xchecker",
        "First-Hour Bootstrap",
        "Brownfield Adoption Packet",
    ] {
        require_contains(&doc, marker, "pilot selection doc")?;
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
