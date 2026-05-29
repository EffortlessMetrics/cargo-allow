use allow_core::{CargoAllowError, CargoAllowResult};
use std::path::{Path, PathBuf};

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
