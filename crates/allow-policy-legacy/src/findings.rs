use allow_core::{
    AllowConfig, AllowEntry, CargoAllowError, CargoAllowResult, Finding, FindingKind, glob_matches,
    normalize_path,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub use crate::finding_config::{network_findings_from_config, process_findings_from_config};
pub(crate) use crate::finding_generated_executable::file_fingerprint;
#[cfg(test)]
pub(crate) use crate::finding_generated_executable::{
    executable_finding, executable_findings_from_git_stage, generated_finding,
};
pub use crate::finding_generated_executable::{
    executable_findings_from_git, generated_findings_from_gitattributes,
};

pub fn workflow_findings_from_files(root: impl AsRef<Path>) -> CargoAllowResult<Vec<Finding>> {
    let root = root.as_ref();
    let workflows_dir = root.join(".github").join("workflows");
    if !workflows_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(&workflows_dir).map_err(|e| {
        CargoAllowError::new(format!("failed to read {}: {e}", workflows_dir.display()))
    })? {
        let entry = entry.map_err(|e| {
            CargoAllowError::new(format!(
                "failed to read {} entry: {e}",
                workflows_dir.display()
            ))
        })?;
        let path = entry.path();
        if is_workflow_path(&path) {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            paths.push(PathBuf::from(rel));
        }
    }
    paths.sort();

    let mut findings = Vec::new();
    for path in paths {
        findings.push(workflow_file_finding(path.clone()));
        let full_path = root.join(
            path.to_string_lossy()
                .replace('/', std::path::MAIN_SEPARATOR_STR),
        );
        let text = fs::read_to_string(&full_path).map_err(|e| {
            CargoAllowError::new(format!("failed to read {}: {e}", full_path.display()))
        })?;
        let uses = text
            .lines()
            .filter_map(extract_workflow_uses)
            .collect::<BTreeSet<_>>();
        findings.extend(
            uses.into_iter()
                .map(|action| workflow_action_finding(path.clone(), action)),
        );
    }
    Ok(findings)
}

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

pub(crate) fn workflow_file_finding(path: PathBuf) -> Finding {
    let normalized = normalize_path(&path);
    let mut identity = allow_core::StructuralIdentity::new("workflow", "github_workflow");
    identity.symbol = Some(normalized);
    Finding {
        kind: FindingKind::PolicyException,
        family: Some("github_workflow".to_string()),
        path,
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity,
        message: "GitHub Actions workflow file".to_string(),
    }
}

pub(crate) fn workflow_action_finding(path: PathBuf, action: String) -> Finding {
    let normalized = normalize_path(&path);
    let mut identity = allow_core::StructuralIdentity::new("workflow", "github_action_uses");
    identity.symbol = Some(workflow_action_symbol(&normalized, &action));
    identity.target_fingerprint = Some(format!("action:{action}"));
    Finding {
        kind: FindingKind::PolicyException,
        family: Some("workflow_external_action".to_string()),
        path,
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity,
        message: format!("GitHub Actions workflow uses external action {action}"),
    }
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

fn extract_workflow_uses(line: &str) -> Option<String> {
    let trimmed = line.trim().trim_start_matches('-').trim_start();
    let stripped = trimmed.strip_prefix("uses:")?;
    let value = stripped.trim();
    if value.is_empty() {
        return None;
    }
    let no_comment = value.split('#').next().unwrap_or(value).trim();
    if no_comment.is_empty() {
        None
    } else {
        Some(no_comment.to_string())
    }
}

pub(crate) fn workflow_action_symbol(path: &str, action: &str) -> String {
    format!("{path} uses {action}")
}

fn is_workflow_path(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("yml" | "yaml")
    )
}
