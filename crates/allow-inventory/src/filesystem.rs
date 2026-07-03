use allow_core::{CargoAllowError, CargoAllowResult};
use std::fs;
use std::path::{Path, PathBuf};

/// Partition `files` into those that still exist on disk as regular files and
/// those that are git-tracked but absent from the worktree (deleted-tracked).
///
/// A deleted-tracked file (still in `git ls-files`, missing on disk) is recorded
/// rather than silently dropped (#2048): the caller surfaces it as an inventory
/// diagnostic so a scan never looks complete while a tracked path disappeared
/// from coverage. Only `NotFound` counts as deleted-tracked; other stat errors
/// (permission denied, etc.) are treated as "not a file" and excluded from both
/// lists, since they belong to the separate inaccessible-file disclosure.
pub(crate) fn existing_regular_files(
    root: &Path,
    files: Vec<PathBuf>,
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut existing = Vec::with_capacity(files.len());
    let mut deleted_tracked = Vec::new();
    for path in files {
        match fs::metadata(root.join(&path)) {
            Ok(metadata) => {
                if metadata.file_type().is_file() {
                    existing.push(path);
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                deleted_tracked.push(path);
            }
            Err(_) => {
                // Other stat errors: treat as not-a-file (excluded), reserved
                // for a separate inaccessible-file diagnostic.
            }
        }
    }
    (existing, deleted_tracked)
}

pub(crate) fn recursive_files(root: &Path) -> CargoAllowResult<Vec<PathBuf>> {
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
            // Resolve the symlink target. Include if it points to a regular
            // file (#1842). Do NOT recurse into symlinked directories (loop
            // safety).
            if fs::metadata(&path)
                .map(|m| m.file_type().is_file())
                .unwrap_or(false)
            {
                let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                out.push(rel);
            }
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
pub(crate) fn visit_for_test(
    root: &Path,
    dir: &Path,
    out: &mut Vec<PathBuf>,
) -> CargoAllowResult<()> {
    visit(root, dir, out)
}
