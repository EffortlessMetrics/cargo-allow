use allow_core::{CargoAllowError, CargoAllowResult};
use rayon::prelude::*;
use std::collections::HashMap;
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
///
/// #4677: git-for-Windows traverses junctions, so `git ls-files` can emit the
/// same real file under a junctioned alias spelling on top of its ordinary
/// spelling. Classifying such a path through the ordinary follow-the-link
/// metadata probes scans the target tree twice — a zero-signal double count —
/// and a junction cycle yields only bounded-but-phantom coverage. Every
/// repo-relative directory prefix is therefore probed once per unique prefix
/// (one extra stat per directory, cached): a prefix that is itself a reparse
/// point resolving to a directory makes the whole path an alias traversal,
/// which is skipped and disclosed instead of scanned.
///
/// Skip-and-disclose rather than canonical-path dedup, deliberately: dedup
/// needs `fs::canonicalize` per file plus verbatim-prefix and broken-link
/// handling, and — decisive — it makes ledger identity alias-dependent. An
/// entry legitimately recorded under `linked_src/lib.rs` would become
/// permanently unmatchable once the junction disappears, recreating the #4678
/// failure mode by design. Disclosure reuses the proven #1917/#4646 typed-skip
/// pattern (typed list -> Partial -> enforcing-mode gate) and fails closed.
///
/// A reparse point in the *final* path component keeps #1842 walk parity
/// (follow the target): std reports any reparse point as `is_symlink`
/// (including cloud-placeholder files), so the component is resolved with the
/// follow-metadata probe — a regular-file target stays `Existing`; a directory
/// or missing target is skipped and disclosed instead of today's wrong
/// `Submodule`/`Deleted` classification. `Submodule` stays strictly for
/// non-reparse directories (#1846).
///
/// Verified std constraint that forces the prefix walk: `symlink_metadata`
/// does not detect *ancestor* reparse points — with only `linked_src` being
/// the junction, `symlink_metadata("linked_src/lib.rs").is_symlink()` is
/// false — so per-path probing alone is insufficient.
pub(crate) fn existing_regular_files(root: &Path, files: Vec<PathBuf>) -> ReparseClassification {
    classify_regular_files(
        root,
        files,
        |path| fs::metadata(path),
        |path| {
            fs::symlink_metadata(path)
                .map(|meta| meta.file_type().is_symlink())
                .unwrap_or(false)
        },
    )
}

#[cfg(test)]
pub(crate) fn existing_regular_files_with_metadata<F, S>(
    root: &Path,
    files: Vec<PathBuf>,
    metadata: F,
    is_reparse_point: S,
) -> ReparseClassification
where
    F: Fn(&Path) -> std::io::Result<fs::Metadata> + Sync,
    S: Fn(&Path) -> bool + Sync,
{
    classify_regular_files(root, files, metadata, is_reparse_point)
}

/// The five-way disposition of the git-provided path list: on-disk regular
/// files, worktree-absent tracked paths, submodule gitlinks, inaccessible
/// paths, and reparse-alias traversals skipped with disclosure (#4677).
pub(crate) struct ReparseClassification {
    pub existing: Vec<PathBuf>,
    pub deleted_tracked: Vec<PathBuf>,
    pub submodule_paths: Vec<PathBuf>,
    pub inaccessible_paths: Vec<PathBuf>,
    pub reparse_skipped_paths: Vec<PathBuf>,
}

fn classify_regular_files<F, S>(
    root: &Path,
    files: Vec<PathBuf>,
    metadata: F,
    is_reparse_point: S,
) -> ReparseClassification
where
    F: Fn(&Path) -> std::io::Result<fs::Metadata> + Sync,
    S: Fn(&Path) -> bool + Sync,
{
    // One alias probe per unique repo-relative directory prefix (#4677). The
    // verdict set is built sequentially and then shared read-only by the
    // parallel classification below.
    let mut alias_dir_prefixes: HashMap<PathBuf, ()> = HashMap::new();
    for path in &files {
        for prefix in path.ancestors().skip(1) {
            if prefix.as_os_str().is_empty() {
                // The empty prefix is the scan root itself, not a traversal.
                continue;
            }
            if alias_dir_prefixes.contains_key(prefix) {
                continue;
            }
            let probe = root.join(prefix);
            let is_alias_dir =
                is_reparse_point(&probe) && metadata(&probe).is_ok_and(|m| m.is_dir());
            if is_alias_dir {
                alias_dir_prefixes.insert(prefix.to_path_buf(), ());
            }
        }
    }
    let mut existing = Vec::with_capacity(files.len());
    let mut deleted_tracked = Vec::new();
    let mut submodule_paths = Vec::new();
    let mut inaccessible_paths = Vec::new();
    let mut reparse_skipped_paths = Vec::new();
    let classified = files
        .into_par_iter()
        .filter_map(|path| {
            if path
                .ancestors()
                .skip(1)
                .any(|prefix| alias_dir_prefixes.contains_key(prefix))
            {
                // The path reaches its content through a junctioned directory
                // spelling: scanning it would double-count the target tree
                // under an alias (#4677).
                return Some(PathDisposition::ReparseSkipped(path));
            }
            let full = root.join(&path);
            match metadata(&full) {
                Ok(metadata) => {
                    if metadata.file_type().is_file() {
                        Some(PathDisposition::Existing(path))
                    } else if metadata.file_type().is_dir() {
                        if is_reparse_point(&full) {
                            // A reparse point resolving to a directory is an
                            // alias traversal, not a submodule (#4677).
                            Some(PathDisposition::ReparseSkipped(path))
                        } else {
                            // A git-tracked path that is a directory on disk is
                            // a checked-out submodule gitlink (#1846).
                            Some(PathDisposition::Submodule(path))
                        }
                    } else {
                        None
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    if is_reparse_point(&full) {
                        // A dangling reparse point is neither a deleted tracked
                        // file nor scannable content; skip + disclose (#4677).
                        Some(PathDisposition::ReparseSkipped(path))
                    } else {
                        Some(PathDisposition::Deleted(path))
                    }
                }
                Err(_) => Some(PathDisposition::Inaccessible(path)),
            }
        })
        .collect::<Vec<_>>();
    for disposition in classified {
        match disposition {
            PathDisposition::Existing(path) => existing.push(path),
            PathDisposition::Deleted(path) => deleted_tracked.push(path),
            PathDisposition::Submodule(path) => submodule_paths.push(path),
            PathDisposition::Inaccessible(path) => inaccessible_paths.push(path),
            PathDisposition::ReparseSkipped(path) => reparse_skipped_paths.push(path),
        }
    }
    ReparseClassification {
        existing,
        deleted_tracked,
        submodule_paths,
        inaccessible_paths,
        reparse_skipped_paths,
    }
}

enum PathDisposition {
    Existing(PathBuf),
    Deleted(PathBuf),
    Submodule(PathBuf),
    Inaccessible(PathBuf),
    ReparseSkipped(PathBuf),
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
                    allow_core::strip_win32_verbatim_prefix(&dir.display().to_string())
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
