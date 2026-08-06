//! Source-tree root discovery and file inventory for cargo-allow.
//!
//! Inventory prefers explicit roots, Git-tracked files when available, and a
//! filesystem fallback otherwise. The crate does not call `cargo metadata` or
//! require a compilable project; Cargo manifests are just files in the scanned
//! source tree.

use allow_core::{CargoAllowResult, source_tree_path_is_ignored};
use std::path::{Path, PathBuf};

mod filesystem;
mod git;
mod options;
mod root;

pub use git::{git_ls_files, git_ls_files_include_untracked, git_worktree_metadata_present};
pub use options::{Inventory, InventoryCompleteness, InventoryOptions, InventorySource};
pub use root::{discover_source_tree_root, resolve_source_tree_root};

use filesystem::{existing_regular_files, recursive_files};

pub use filesystem::{INVENTORY_MAX_DEPTH, INVENTORY_MAX_ENTRIES};
#[cfg(test)]
pub(crate) use git::parse_git_ls_files_z;

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
    let (
        mut files,
        source,
        empty_git_tracked,
        deleted_tracked,
        git_error,
        skipped_paths,
        submodule_paths,
    ) = if options.include_untracked {
        // Prefer git's own .gitignore rules when available (#1843). Fall
        // back to raw filesystem walk only when not in a git repo.
        match git_ls_files_include_untracked(root) {
            Ok(files) => {
                let (existing, deleted, submods) = existing_regular_files(root, files);
                (
                    existing,
                    InventorySource::FilesystemIncludeUntracked,
                    false,
                    deleted,
                    None,
                    Vec::new(),
                    submods,
                )
            }
            Err(err) => {
                let (files, skipped) = recursive_files(root)?;
                (
                    files,
                    InventorySource::FilesystemIncludeUntracked,
                    false,
                    Vec::new(),
                    Some(err.to_string()),
                    skipped,
                    Vec::new(),
                )
            }
        }
    } else {
        match git_ls_files(root) {
            Ok(files) => {
                let empty_git_tracked = files.is_empty();
                let (existing, deleted_tracked, submodule_paths) =
                    existing_regular_files(root, files);
                (
                    existing,
                    InventorySource::GitTracked,
                    empty_git_tracked,
                    deleted_tracked,
                    None,
                    Vec::new(),
                    submodule_paths,
                )
            }
            Err(err) => {
                let (files, skipped) = recursive_files(root)?;
                (
                    files,
                    InventorySource::FilesystemFallback,
                    false,
                    Vec::new(),
                    Some(err.to_string()),
                    skipped,
                    Vec::new(),
                )
            }
        }
    };
    files.sort();
    files.dedup();
    files.retain(|path| !source_tree_path_is_ignored(path, &options.ignored));
    let completeness = if git_error.is_some() {
        options::InventoryCompleteness::Fallback
    } else if !deleted_tracked.is_empty()
        || !submodule_paths.is_empty()
        || !skipped_paths.is_empty()
    {
        options::InventoryCompleteness::Partial
    } else if !options.ignored.is_empty() || !options.generated.is_empty() {
        options::InventoryCompleteness::Scoped
    } else {
        options::InventoryCompleteness::Complete
    };
    Ok(Inventory {
        files,
        source,
        completeness,
        empty_git_tracked,
        deleted_tracked,
        git_error,
        skipped_paths,
        submodule_paths,
    })
}

#[cfg(test)]
mod tests;
