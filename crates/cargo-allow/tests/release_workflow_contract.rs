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

/// Drift guard for #3915 PR C: every platform row of the release-set lane
/// must run allow-rust's persistent scan-cache suites. A platform-wide
/// `--skip` exclusion (the pre-PR-C macOS workaround) must not silently
/// return, and the lane manifest must not re-describe the suite as bounded.
#[test]
fn test_ci_workflow_runs_the_full_cache_suite_on_every_platform() -> Result<(), Box<dyn Error>> {
    let root = repo_root()?;
    let ci_path = root.join(".github/workflows/ci.yml");
    if !ci_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&ci_path)?;

    for skipped in [
        "--skip persistent_scan_cache",
        "--skip scan_cache_store",
        "cache_scope: bounded",
    ] {
        require(
            !content.contains(skipped),
            &format!("ci.yml must not reintroduce the platform cache exclusion: {skipped}"),
        )?;
    }

    require(
        content.contains("-p allow-rust"),
        "ci.yml must keep the allow-rust release-set suite",
    )?;

    let lanes_path = root.join("docs/ci-lanes.toml");
    if lanes_path.exists() {
        let lanes = fs::read_to_string(&lanes_path)?;
        require(
            !lanes.contains("cache exclusions remain bounded"),
            "ci-lanes.toml must not re-describe the cache suite as bounded",
        )?;
    }

    Ok(())
}
