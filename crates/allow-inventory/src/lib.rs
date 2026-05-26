use allow_core::{CargoAllowError, CargoAllowResult, glob_matches, normalize_path};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct InventoryOptions {
    pub ignored: Vec<String>,
    pub generated: Vec<String>,
    pub include_untracked: bool,
}

impl Default for InventoryOptions {
    fn default() -> Self {
        Self {
            ignored: vec![".git/**".to_string(), "target/**".to_string()],
            generated: Vec::new(),
            include_untracked: false,
        }
    }
}

pub fn discover_workspace_root(start: impl AsRef<Path>) -> CargoAllowResult<PathBuf> {
    let mut dir = start
        .as_ref()
        .canonicalize()
        .map_err(|e| CargoAllowError::new(format!("failed to canonicalize start path: {e}")))?;
    loop {
        if dir.join("Cargo.toml").exists() || dir.join(".git").exists() {
            return Ok(dir);
        }
        if !dir.pop() {
            return Err(CargoAllowError::new(
                "could not find Cargo.toml or .git in this directory or ancestors",
            ));
        }
    }
}

pub fn inventory_files(
    root: impl AsRef<Path>,
    options: &InventoryOptions,
) -> CargoAllowResult<Vec<PathBuf>> {
    let root = root.as_ref();
    let mut files = if options.include_untracked {
        recursive_files(root)?
    } else {
        git_ls_files(root).unwrap_or_else(|_| recursive_files(root).unwrap_or_default())
    };
    files.sort();
    files.dedup();
    files.retain(|path| !is_ignored(path, &options.ignored));
    Ok(files)
}

pub fn git_ls_files(root: impl AsRef<Path>) -> CargoAllowResult<Vec<PathBuf>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root.as_ref())
        .arg("ls-files")
        .output()
        .map_err(|e| CargoAllowError::new(format!("failed to invoke git ls-files: {e}")))?;
    if !output.status.success() {
        return Err(CargoAllowError::new("git ls-files failed"));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(PathBuf::from)
        .collect())
}

fn recursive_files(root: &Path) -> CargoAllowResult<Vec<PathBuf>> {
    let mut out = Vec::new();
    visit(root, root, &mut out)?;
    Ok(out)
}

fn visit(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> CargoAllowResult<()> {
    for entry in fs::read_dir(dir)
        .map_err(|e| CargoAllowError::new(format!("failed to read {}: {e}", dir.display())))?
    {
        let entry = entry
            .map_err(|e| CargoAllowError::new(format!("failed to read directory entry: {e}")))?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == ".git" || name == "target" {
            continue;
        }
        if path.is_dir() {
            visit(root, &path, out)?;
        } else if path.is_file() {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            out.push(rel);
        }
    }
    Ok(())
}

fn is_ignored(path: &Path, patterns: &[String]) -> bool {
    let normalized = normalize_path(path);
    patterns.iter().any(|pattern| {
        glob_matches(pattern, path)
            || pattern
                .strip_suffix("/**")
                .map(|prefix| normalized == prefix || normalized.starts_with(&format!("{prefix}/")))
                .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_target_paths() {
        let opts = InventoryOptions::default();
        assert!(super::is_ignored(
            Path::new("target/debug/x"),
            &opts.ignored
        ));
    }

    #[test]
    fn dot_git_ignore_does_not_swallow_dot_github() {
        let opts = InventoryOptions::default();
        assert!(!super::is_ignored(
            Path::new(".github/workflows/ci.yml"),
            &opts.ignored
        ));
    }
}
