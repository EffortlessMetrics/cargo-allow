use allow_core::{CargoAllowError, CargoAllowResult, source_tree_path_is_ignored};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventorySource {
    GitTracked,
    FilesystemFallback,
    FilesystemIncludeUntracked,
}

impl InventorySource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GitTracked => "git_tracked",
            Self::FilesystemFallback => "filesystem_fallback",
            Self::FilesystemIncludeUntracked => "filesystem_include_untracked",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    pub files: Vec<PathBuf>,
    pub source: InventorySource,
}

pub fn resolve_source_tree_root(
    explicit_root: Option<&Path>,
    start: impl AsRef<Path>,
) -> CargoAllowResult<PathBuf> {
    if let Some(root) = explicit_root {
        return canonical_dir(root);
    }
    discover_source_tree_root(start)
}

pub fn discover_source_tree_root(start: impl AsRef<Path>) -> CargoAllowResult<PathBuf> {
    let start = canonical_start_dir(start.as_ref())?;
    let mut dir = start.as_path();
    loop {
        if dir.join(".git").exists() {
            return Ok(dir.to_path_buf());
        }
        let Some(parent) = dir.parent() else {
            return Ok(start);
        };
        dir = parent;
    }
}

fn canonical_dir(path: &Path) -> CargoAllowResult<PathBuf> {
    let canonical = path
        .canonicalize()
        .map_err(|e| CargoAllowError::new(format!("failed to canonicalize root path: {e}")))?;
    if canonical.is_dir() {
        Ok(canonical)
    } else {
        Err(CargoAllowError::new(format!(
            "source tree root is not a directory: {}",
            canonical.display()
        )))
    }
}

fn canonical_start_dir(start: &Path) -> CargoAllowResult<PathBuf> {
    let canonical = start
        .canonicalize()
        .map_err(|e| CargoAllowError::new(format!("failed to canonicalize start path: {e}")))?;
    if canonical.is_file() {
        canonical
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| CargoAllowError::new("start path has no parent directory"))
    } else {
        Ok(canonical)
    }
}

pub fn inventory_files(
    root: impl AsRef<Path>,
    options: &InventoryOptions,
) -> CargoAllowResult<Vec<PathBuf>> {
    Ok(inventory(root, options)?.files)
}

pub fn inventory(
    root: impl AsRef<Path>,
    options: &InventoryOptions,
) -> CargoAllowResult<Inventory> {
    let root = root.as_ref();
    let (mut files, source) = if options.include_untracked {
        (
            recursive_files(root)?,
            InventorySource::FilesystemIncludeUntracked,
        )
    } else {
        match git_ls_files(root) {
            Ok(files) => (
                existing_regular_files(root, files),
                InventorySource::GitTracked,
            ),
            Err(_) => (recursive_files(root)?, InventorySource::FilesystemFallback),
        }
    };
    files.sort();
    files.dedup();
    files.retain(|path| !source_tree_path_is_ignored(path, &options.ignored));
    Ok(Inventory { files, source })
}

fn existing_regular_files(root: &Path, files: Vec<PathBuf>) -> Vec<PathBuf> {
    files
        .into_iter()
        .filter(|path| {
            fs::symlink_metadata(root.join(path))
                .map(|metadata| metadata.file_type().is_file())
                .unwrap_or(false)
        })
        .collect()
}

pub fn git_ls_files(root: impl AsRef<Path>) -> CargoAllowResult<Vec<PathBuf>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root.as_ref())
        .arg("ls-files")
        .arg("-z")
        .output()
        .map_err(|e| CargoAllowError::new(format!("failed to invoke git ls-files: {e}")))?;
    if !output.status.success() {
        return Err(CargoAllowError::new("git ls-files failed"));
    }
    Ok(parse_git_ls_files_z(&output.stdout))
}

fn parse_git_ls_files_z(stdout: &[u8]) -> Vec<PathBuf> {
    stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| PathBuf::from(String::from_utf8_lossy(path).into_owned()))
        .collect()
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
        let file_type = entry.file_type().map_err(|e| {
            CargoAllowError::new(format!("failed to inspect {}: {e}", path.display()))
        })?;
        if file_type.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == ".git" || name == "target" {
            continue;
        }
        if file_type.is_dir() {
            visit(root, &path, out)?;
        } else if file_type.is_file() {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            out.push(rel);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
