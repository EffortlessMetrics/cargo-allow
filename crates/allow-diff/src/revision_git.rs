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
        .arg("--name-only")
        .arg(revision)
        .output()
        .map_err(|e| CargoAllowError::new(format!("failed to run git ls-tree: {e}")))?;
    if !output.status.success() {
        return Err(CargoAllowError::new(format!(
            "git ls-tree failed for {revision}"
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(PathBuf::from)
        .collect())
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
