use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, FindingKind, glob_matches,
    normalize_path,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn dependency_surface_findings_from_git(
    root: impl AsRef<Path>,
    cfg: &AllowConfig,
) -> CargoAllowResult<Vec<Finding>> {
    let tracked = git_ls_files(root)?;
    let mut paths = BTreeSet::new();
    for entry in &cfg.allow {
        if entry.kind != FindingKind::PolicyException
            || entry.family.as_deref() != Some("dependency_surface")
        {
            continue;
        }
        for path in &tracked {
            if dependency_entry_matches_path(entry, path) {
                paths.insert(path.clone());
            }
        }
    }
    Ok(paths.into_iter().map(dependency_surface_finding).collect())
}

fn git_ls_files(root: impl AsRef<Path>) -> CargoAllowResult<Vec<PathBuf>> {
    let output = Command::new("git")
        .args(["ls-files"])
        .current_dir(root.as_ref())
        .output()
        .map_err(|e| CargoAllowError::new(format!("failed to run git ls-files: {e}")))?;
    if !output.status.success() {
        return Err(CargoAllowError::new(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|e| CargoAllowError::new(format!("git ls-files output was not UTF-8: {e}")))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect())
}

pub(crate) fn dependency_surface_finding(path: PathBuf) -> Finding {
    let normalized = normalize_path(&path);
    let mut identity = allow_core::StructuralIdentity::new("file", "dependency_surface");
    identity.symbol = Some(normalized.clone());
    identity.target_fingerprint = Some(dependency_surface_family(&path));
    Finding {
        kind: FindingKind::PolicyException,
        family: Some("dependency_surface".to_string()),
        path,
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity,
        message: format!("tracked dependency surface {normalized}"),
    }
}

fn dependency_surface_family(path: &Path) -> String {
    let normalized = normalize_path(path);
    match normalized.as_str() {
        "Cargo.toml" => "workspace_manifest".to_string(),
        "Cargo.lock" => "workspace_lockfile".to_string(),
        "rust-toolchain.toml" => "toolchain_pin".to_string(),
        "deny.toml" => "policy_config".to_string(),
        text if text.ends_with("/Cargo.toml") => "crate_manifest".to_string(),
        text if text.ends_with("/Cargo.lock") => "lockfile".to_string(),
        text if text.ends_with("/rust-toolchain.toml") => "toolchain_pin".to_string(),
        _ => "dependency_surface".to_string(),
    }
}

fn dependency_entry_matches_path(entry: &AllowEntry, path: &Path) -> bool {
    entry
        .path
        .as_ref()
        .is_some_and(|scope| normalize_path(scope) == normalize_path(path))
        || entry
            .glob
            .as_ref()
            .is_some_and(|glob| glob_matches(glob, path))
        || entry
            .selector
            .glob
            .as_ref()
            .is_some_and(|glob| glob_matches(glob, path))
}
