use allow_core::{CargoAllowError, CargoAllowResult, Finding, FindingKind, normalize_path};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn generated_findings_from_gitattributes(
    root: impl AsRef<Path>,
) -> CargoAllowResult<Vec<Finding>> {
    let path = root.as_ref().join(".gitattributes");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text = fs::read_to_string(&path)
        .map_err(|e| CargoAllowError::new(format!("failed to read {}: {e}", path.display())))?;
    Ok(generated_paths_from_gitattributes(&text)
        .into_iter()
        .map(generated_finding)
        .collect())
}

pub fn executable_findings_from_git(root: impl AsRef<Path>) -> CargoAllowResult<Vec<Finding>> {
    let output = Command::new("git")
        .args(["ls-files", "--stage"])
        .current_dir(root.as_ref())
        .output()
        .map_err(|e| CargoAllowError::new(format!("failed to run git ls-files --stage: {e}")))?;
    if !output.status.success() {
        return Err(CargoAllowError::new(format!(
            "git ls-files --stage failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|e| CargoAllowError::new(format!("git ls-files output was not UTF-8: {e}")))?;
    Ok(executable_findings_from_git_stage(&text))
}

fn generated_paths_from_gitattributes(input: &str) -> Vec<PathBuf> {
    input
        .lines()
        .filter_map(generated_path_from_gitattributes_line)
        .map(PathBuf::from)
        .collect()
}

fn generated_path_from_gitattributes_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || !trimmed.contains("linguist-generated=true")
    {
        return None;
    }
    trimmed
        .split_whitespace()
        .next()
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_string)
}

pub(crate) fn generated_finding(path: PathBuf) -> Finding {
    let normalized = normalize_path(&path);
    let mut identity = allow_core::StructuralIdentity::new("file", "tracked_file");
    identity.symbol = Some(normalized);
    identity.target_fingerprint = file_fingerprint(&path);
    Finding {
        kind: FindingKind::GeneratedCode,
        family: Some("generated_code".to_string()),
        path,
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity,
        message: "tracked generated file from .gitattributes".to_string(),
    }
}

pub(crate) fn executable_findings_from_git_stage(input: &str) -> Vec<Finding> {
    input
        .lines()
        .filter_map(executable_path_from_git_stage_line)
        .map(executable_finding)
        .collect()
}

fn executable_path_from_git_stage_line(line: &str) -> Option<PathBuf> {
    let (meta, path) = line.split_once('\t')?;
    let mode = meta.split_whitespace().next()?;
    if mode == "100755" && !path.trim().is_empty() {
        Some(PathBuf::from(path.trim()))
    } else {
        None
    }
}

pub(crate) fn executable_finding(path: PathBuf) -> Finding {
    let normalized = normalize_path(&path);
    let mut identity = allow_core::StructuralIdentity::new("file", "git_executable_file");
    identity.symbol = Some(normalized);
    identity.target_fingerprint = Some("git-mode:100755".to_string());
    Finding {
        kind: FindingKind::PolicyException,
        family: Some("executable_file".to_string()),
        path,
        span: Some(allow_core::Span { line: 1, column: 1 }),
        identity,
        message: "tracked file has git executable bit".to_string(),
    }
}

pub(crate) fn file_fingerprint(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
        .or_else(|| {
            path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_ascii_lowercase())
        })
}
