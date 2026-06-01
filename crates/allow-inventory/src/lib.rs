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

pub use git::git_ls_files;
pub use options::{Inventory, InventoryOptions, InventorySource};
pub use root::{discover_source_tree_root, resolve_source_tree_root};

use filesystem::{existing_regular_files, recursive_files};

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

#[cfg(test)]
mod tests;
