use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const EXPECTED_CARGO_ALLOW_CRATES: &[&str] = &[
    "allow-core",
    "allow-policy",
    "allow-policy-legacy",
    "allow-inventory",
    "allow-files",
    "allow-rust",
    "allow-match",
    "allow-report",
    "allow-diff",
    "cargo-allow",
];

const EXPECTED_SHARED_PREREQUISITES: &[&str] = &[
    "effortless-repo-protocol",
    "effortless-repo-snapshot",
    "effortless-repo-edit",
];

fn require(cond: bool, msg: &str) -> Result<(), io::Error> {
    if !cond {
        Err(io::Error::other(msg))
    } else {
        Ok(())
    }
}

#[test]
fn release_package_metadata_and_topology_alignment() -> Result<(), Box<dyn Error>> {
    let root = repository_root()?;
    if !root.join(".git").exists() {
        return Ok(());
    }

    let root_toml_content = fs::read_to_string(root.join("Cargo.toml"))?;
    let root_toml: toml::Value = toml::from_str(&root_toml_content)?;
    let root_json = serde_json::to_value(&root_toml)?;

    let workspace_version = root_json
        .pointer("/workspace/package/version")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| io::Error::other("missing workspace package version"))?;

    for crate_name in EXPECTED_CARGO_ALLOW_CRATES {
        let manifest_path = root.join("crates").join(crate_name).join("Cargo.toml");
        require(
            manifest_path.exists(),
            &format!(
                "manifest must exist for {crate_name}: {}",
                manifest_path.display()
            ),
        )?;
        let crate_content = fs::read_to_string(&manifest_path)?;
        let crate_toml: toml::Value = toml::from_str(&crate_content)?;
        let crate_json = serde_json::to_value(&crate_toml)?;

        let pkg_version = crate_json
            .pointer("/package/version")
            .and_then(serde_json::Value::as_str);
        let pkg_version_workspace = crate_json
            .pointer("/package/version/workspace")
            .and_then(serde_json::Value::as_bool);

        if let Some(v) = pkg_version {
            require(
                v == workspace_version,
                &format!(
                    "crate {crate_name} explicit version must equal workspace version {workspace_version}"
                ),
            )?;
        } else {
            require(
                pkg_version_workspace == Some(true),
                &format!("crate {crate_name} must inherit workspace version"),
            )?;
        }
    }

    for shared_name in EXPECTED_SHARED_PREREQUISITES {
        let manifest_path = root.join("crates").join(shared_name).join("Cargo.toml");
        require(
            manifest_path.exists(),
            &format!(
                "manifest must exist for {shared_name}: {}",
                manifest_path.display()
            ),
        )?;
        let crate_content = fs::read_to_string(&manifest_path)?;
        let crate_toml: toml::Value = toml::from_str(&crate_content)?;
        let crate_json = serde_json::to_value(&crate_toml)?;
        let pkg_version = crate_json
            .pointer("/package/version")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| io::Error::other(format!("missing version for {shared_name}")))?;
        require(
            pkg_version == "0.1.0",
            &format!("shared prerequisite {shared_name} must maintain independent 0.1.0 version"),
        )?;
    }

    Ok(())
}

#[test]
fn exact_internal_dependency_pinning_in_workspace() -> Result<(), Box<dyn Error>> {
    let root = repository_root()?;
    if !root.join(".git").exists() {
        return Ok(());
    }

    let root_toml_content = fs::read_to_string(root.join("Cargo.toml"))?;
    let root_toml: toml::Value = toml::from_str(&root_toml_content)?;
    let root_json = serde_json::to_value(&root_toml)?;

    let deps = root_json
        .pointer("/workspace/dependencies")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| io::Error::other("missing [workspace.dependencies]"))?;

    for crate_name in EXPECTED_CARGO_ALLOW_CRATES {
        if *crate_name == "cargo-allow" {
            continue;
        }
        let dep = deps.get(*crate_name).ok_or_else(|| {
            io::Error::other(format!("missing {crate_name} in [workspace.dependencies]"))
        })?;
        let version_str = dep
            .get("version")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| io::Error::other(format!("missing version for {crate_name}")))?;

        require(
            version_str.starts_with('='),
            &format!(
                "internal dependency {crate_name} version must be exact '=' pinned, got: {version_str}"
            ),
        )?;
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
