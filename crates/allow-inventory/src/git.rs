use allow_core::{CargoAllowError, CargoAllowResult};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn git_ls_files(root: impl AsRef<Path>) -> CargoAllowResult<Vec<PathBuf>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root.as_ref())
        .arg("ls-files")
        .arg("-z")
        .output()
        .map_err(|e| CargoAllowError::new(format!("failed to invoke git ls-files: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let code = output.status.code().unwrap_or(-1);
        return Err(CargoAllowError::new(format!(
            "git ls-files failed (exit {code}): {stderr}{}",
            if stderr.contains("not a git repository") || stderr.contains("fatal: not a git") {
                "; this directory may not be a git repository — run `git init` or use `--inventory filesystem` to scan without git"
            } else {
                ""
            }
        )));
    }
    Ok(parse_git_ls_files_z(&output.stdout))
}

/// Like `git_ls_files` but also includes untracked files while respecting
/// `.gitignore`, `.git/info/exclude`, and the global excludesfile. Uses
/// `git ls-files --others --exclude-standard --cached -z` so build artifacts
/// listed in `.gitignore` are excluded (#1843).
pub fn git_ls_files_include_untracked(root: impl AsRef<Path>) -> CargoAllowResult<Vec<PathBuf>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root.as_ref())
        .arg("ls-files")
        .arg("--others")
        .arg("--exclude-standard")
        .arg("--cached")
        .arg("-z")
        .output()
        .map_err(|e| {
            CargoAllowError::new(format!("failed to invoke git ls-files --others: {e}"))
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let code = output.status.code().unwrap_or(-1);
        return Err(CargoAllowError::new(format!(
            "git ls-files --others --exclude-standard failed (exit {code}): {stderr}"
        )));
    }
    Ok(parse_git_ls_files_z(&output.stdout))
}

pub(crate) fn parse_git_ls_files_z(stdout: &[u8]) -> Vec<PathBuf> {
    stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(bytes_to_path)
        .collect()
}

/// Convert raw bytes from `git ls-files -z` to a `PathBuf`.
///
/// On Unix, paths are arbitrary bytes (`OsStr`), so we construct the path
/// directly from the raw bytes without UTF-8 validation. This preserves
/// non-UTF-8 filenames (Latin-1, mojibake, etc.) that `from_utf8_lossy`
/// would corrupt with `U+FFFD` replacements (#1841).
///
/// On Windows, git emits WTF-8/UTF-8 encoded paths, so the lossy conversion
/// is the correct fallback.
#[cfg(unix)]
fn bytes_to_path(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
fn bytes_to_path(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}
