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
