use allow_core::{CargoAllowError, CargoAllowResult};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn changed_files(
    root: impl AsRef<Path>,
    base: &str,
    head: Option<&str>,
) -> CargoAllowResult<Vec<PathBuf>> {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(root.as_ref())
        .arg("diff")
        .arg("--name-only")
        .arg(base);
    if let Some(head) = head {
        cmd.arg(head);
    }
    let output = cmd
        .output()
        .map_err(|e| CargoAllowError::new(format!("failed to run git diff: {e}")))?;
    if !output.status.success() {
        return Err(CargoAllowError::new("git diff --name-only failed"));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(PathBuf::from)
        .collect())
}

pub fn git_tracked_files_at_revision(
    root: impl AsRef<Path>,
    revision: &str,
) -> CargoAllowResult<Vec<PathBuf>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root.as_ref())
        .arg("ls-tree")
        .arg("-r")
        .arg("-z")
        .arg(revision)
        .output()
        .map_err(|e| CargoAllowError::new(format!("failed to run git ls-tree: {e}")))?;
    if !output.status.success() {
        return Err(CargoAllowError::new(format!(
            "git ls-tree failed for {revision}"
        )));
    }
    Ok(parse_git_ls_tree_z(&output.stdout))
}

pub fn read_file_at_revision(
    root: impl AsRef<Path>,
    revision: &str,
    path: impl AsRef<Path>,
) -> CargoAllowResult<Option<String>> {
    let spec = format!(
        "{}:{}",
        revision,
        path.as_ref().to_string_lossy().replace('\\', "/")
    );
    let output = Command::new("git")
        .arg("-C")
        .arg(root.as_ref())
        .arg("show")
        .arg(&spec)
        .output()
        .map_err(|e| CargoAllowError::new(format!("failed to run git show: {e}")))?;
    if output.status.success() {
        return Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("exists on disk, but not in")
        || stderr.contains("Path")
        || stderr.contains("does not exist")
    {
        return Ok(None);
    }
    Err(CargoAllowError::new(format!(
        "failed to read {} from {revision}",
        path.as_ref().display()
    )))
}

pub(crate) fn parse_git_ls_tree_z(stdout: &[u8]) -> Vec<PathBuf> {
    stdout
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .filter_map(parse_git_ls_tree_record)
        .collect()
}

fn parse_git_ls_tree_record(record: &[u8]) -> Option<PathBuf> {
    let record = String::from_utf8_lossy(record);
    let (metadata, path) = record.split_once('\t')?;
    let mode = metadata.split_whitespace().next()?;
    if !mode.starts_with("100") {
        return None;
    }
    Some(PathBuf::from(path))
}
