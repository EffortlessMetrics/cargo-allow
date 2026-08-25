use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn require(cond: bool, msg: &str) -> Result<(), io::Error> {
    if !cond {
        Err(io::Error::other(msg))
    } else {
        Ok(())
    }
}

fn repo_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = manifest_dir
        .parent()
        .ok_or_else(|| io::Error::other("no crates dir parent"))?;
    let root = crates_dir
        .parent()
        .ok_or_else(|| io::Error::other("no repo root"))?;
    Ok(root.to_path_buf())
}

#[test]
fn test_release_workflow_structure() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let release_wf_path = root.join(".github/workflows/release.yml");
    if !release_wf_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(release_wf_path)?;

    // Must have workflow_dispatch trigger
    require(
        content.contains("workflow_dispatch:"),
        "release.yml must support workflow_dispatch",
    )?;

    // Must not contain hardcoded publish tokens in cleartext
    require(
        !content.contains("CARGO_REGISTRY_TOKEN: \\\""),
        "cleartext CARGO_REGISTRY_TOKEN is prohibited",
    )?;

    Ok(())
}
