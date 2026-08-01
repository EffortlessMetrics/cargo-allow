use allow_core::{CargoAllowError, CargoAllowResult};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

/// Maximum directory nesting depth for filesystem inventory walks.
///
/// Counts path segments under the inventory root (root itself is depth 0).
/// Pathological deep trees stop recursion and record a skip diagnostic (#1917).
pub const INVENTORY_MAX_DEPTH: usize = 64;

/// Maximum regular files collected during one filesystem inventory walk.
///
/// When exceeded, the walk stops and records a skip diagnostic so completeness
/// becomes partial rather than unbounded memory growth (#1917).
pub const INVENTORY_MAX_ENTRIES: usize = 250_000;

/// Partition `files` into those that still exist on disk as regular files and
/// those that are git-tracked but absent from the worktree (deleted-tracked).
///
/// A deleted-tracked file (still in `git ls-files`, missing on disk) is recorded
/// rather than silently dropped (#2048): the caller surfaces it as an inventory
/// diagnostic so a scan never looks complete while a tracked path disappeared
/// from coverage. Only `NotFound` counts as deleted-tracked; other stat errors
/// (permission denied, etc.) are treated as "not a file" and excluded from both
/// lists, since they belong to the separate inaccessible-file disclosure.
///
/// A tracked path that exists as a *directory* (not a file) is recorded as a
/// submodule candidate (#1846): git ls-files reports submodule gitlinks as
/// plain paths, but checked-out submodules are directories on disk.
pub(crate) fn existing_regular_files(
    root: &Path,
    files: Vec<PathBuf>,
) -> (Vec<PathBuf>, Vec<PathBuf>, Vec<PathBuf>) {
    let mut existing = Vec::with_capacity(files.len());
    let mut deleted_tracked = Vec::new();
    let mut submodule_paths = Vec::new();
    let classified = files
        .into_par_iter()
        .filter_map(|path| match fs::metadata(root.join(&path)) {
            Ok(metadata) => {
                if metadata.file_type().is_file() {
                    Some(PathDisposition::Existing(path))
                } else if metadata.file_type().is_dir() {
                    // A git-tracked path that is a directory on disk is a
                    // checked-out submodule gitlink (#1846).
                    Some(PathDisposition::Submodule(path))
                } else {
                    None
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Some(PathDisposition::Deleted(path))
            }
            Err(_) => {
                // Other stat errors: treat as not-a-file (excluded), reserved
                // for a separate inaccessible-file diagnostic.
                None
            }
        })
        .collect::<Vec<_>>();
    for disposition in classified {
        match disposition {
            PathDisposition::Existing(path) => existing.push(path),
            PathDisposition::Deleted(path) => deleted_tracked.push(path),
            PathDisposition::Submodule(path) => submodule_paths.push(path),
        }
    }
    (existing, deleted_tracked, submodule_paths)
}

enum PathDisposition {
    Existing(PathBuf),
    Deleted(PathBuf),
    Submodule(PathBuf),
}

pub(crate) fn recursive_files(root: &Path) -> CargoAllowResult<(Vec<PathBuf>, Vec<PathBuf>)> {
    let mut out = Vec::new();
    let mut skipped = Vec::new();
    // Top-level read_dir failure is a hard error (the root itself is
    // inaccessible). Sub-directory failures are warnings (skip + record).
    visit(root, root, 0, &mut out, &mut skipped)?;
    Ok((out, skipped))
}

fn visit(
    root: &Path,
    dir: &Path,
    depth: usize,
    out: &mut Vec<PathBuf>,
    skipped: &mut Vec<PathBuf>,
) -> CargoAllowResult<()> {
    if depth > INVENTORY_MAX_DEPTH {
        let rel = dir.strip_prefix(root).unwrap_or(dir).to_path_buf();
        skipped.push(rel.join(format!(
            ".cargo-allow-inventory-depth-limit-{INVENTORY_MAX_DEPTH}"
        )));
        return Ok(());
    }
    if out.len() >= INVENTORY_MAX_ENTRIES {
        skipped.push(PathBuf::from(format!(
            ".cargo-allow-inventory-entry-limit-{INVENTORY_MAX_ENTRIES}"
        )));
        return Ok(());
    }
    let dir_entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            // The root directory must be readable; a sub-directory failure is
            // a warning (skip + record) so one permission-denied branch does
            // not abort the entire walk (#1844).
            if dir == root {
                return Err(CargoAllowError::new(format!(
                    "failed to read {}: {e}",
                    dir.display()
                )));
            }
            let rel = dir.strip_prefix(root).unwrap_or(dir).to_path_buf();
            skipped.push(rel);
            return Ok(());
        }
    };
    for entry in dir_entries {
        if out.len() >= INVENTORY_MAX_ENTRIES {
            skipped.push(PathBuf::from(format!(
                ".cargo-allow-inventory-entry-limit-{INVENTORY_MAX_ENTRIES}"
            )));
            return Ok(());
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                // A single unreadable entry in a readable directory is a
                // warning, not a hard failure (#1844).
                let path = dir.join(e.to_string());
                let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                skipped.push(rel);
                continue;
            }
        };
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => {
                let rel = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                skipped.push(rel);
                continue;
            }
        };
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
        if name == ".git" || (name == "target" && dir == root) {
            continue;
        }
        if file_type.is_dir() {
            visit(root, &path, depth + 1, out, skipped)?;
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
    let mut skipped = Vec::new();
    visit(root, dir, 0, out, &mut skipped)
}

#[cfg(test)]
pub(crate) fn visit_for_test_with_depth(
    root: &Path,
    dir: &Path,
    depth: usize,
    out: &mut Vec<PathBuf>,
    skipped: &mut Vec<PathBuf>,
) -> CargoAllowResult<()> {
    visit(root, dir, depth, out, skipped)
}
