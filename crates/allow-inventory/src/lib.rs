//! Source-tree root discovery and file inventory for cargo-allow.
//!
//! Inventory prefers explicit roots, Git-tracked files when available, and a
//! filesystem fallback otherwise. The crate does not call `cargo metadata` or
//! require a compilable project; Cargo manifests are just files in the scanned
//! source tree. The unused-dependency module adds the advisory feature-aware
//! inventory contracts (#3909) over caller-supplied manifest and source texts,
//! and the CI cache experiment module adds the typed contract and measurement
//! laws the hosted Linux cache evidence is graded against (#3963).

use allow_core::{CargoAllowResult, source_tree_path_is_ignored};
use std::path::{Path, PathBuf};

mod ci_cache_experiment;
mod filesystem;
mod git;
mod options;
mod root;
mod unused_dependency;

/// Git-backed file listing plus a locale-independent worktree metadata probe.
pub use ci_cache_experiment::{
    CI_CACHE_EXPERIMENT_V1_CLAIM_BOUNDARY, CI_CACHE_EXPERIMENT_V1_DEFAULT_ROLLBACK_ROUTE,
    CI_CACHE_EXPERIMENT_V1_SCHEMA_ID, CI_CACHE_EXPERIMENT_V1_SCHEMA_VERSION, CachePostureV1,
    CacheRunRecordV1, CacheSaveAuthorityV1, CacheTrustClassV1, CiCacheExperimentV1,
    ExperimentVerdictV1, PINNED_RUST_CACHE_ACTION_REF, REQUIRED_ACCEPTANCE_POSTURES,
    compile_experiment, declared_experiment_limitations, derive_verdict,
    derive_verdict_with_reasons, duration_percentiles, group_runs_by_proof_lane,
    improvement_attribution_note, proof_divergences, render_ci_cache_experiment_v1,
    untrusted_save_violations, validate_experiment, validate_run_record,
};
pub use git::{git_ls_files, git_ls_files_include_untracked, git_worktree_metadata_present};
pub use options::{Inventory, InventoryCompleteness, InventoryOptions, InventorySource};
pub use root::{discover_source_tree_root, resolve_source_tree_root};
pub use unused_dependency::{
    INCOMPLETE_SCAN_EVIDENCE_MARKER, UNUSED_DEPENDENCY_ANALYZER_IDENTITY,
    UNUSED_DEPENDENCY_CLAIM_BOUNDARY, UNUSED_DEPENDENCY_RECEIPT_V1_SCHEMA_ID,
    UNUSED_DEPENDENCY_RECEIPT_V1_SCHEMA_VERSION, UnusedDependencyDependencyClassV1,
    UnusedDependencyDispositionV1, UnusedDependencyExceptionV1, UnusedDependencyFindingV1,
    UnusedDependencyInstrumentPostureV1, UnusedDependencyLibIdentityV1,
    UnusedDependencyManifestRowV1, UnusedDependencyReceiptV1, UnusedDependencyRequestV1,
    UnusedDependencySourceInputV1, declared_absence_limitation, declared_unscanned_kinds,
    empty_receipt, inventory_packages, inventory_unused_dependencies, receipt_scan_is_complete,
    render_unused_dependency_receipt_v1, validate_exception, validate_receipt,
};

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
        inaccessible_paths,
    ) = if options.include_untracked {
        // Prefer git's own .gitignore rules when available (#1843). Fall
        // back to raw filesystem walk only when not in a git repo.
        match git_ls_files_include_untracked(root) {
            Ok(files) => {
                let (existing, deleted, submods, inaccessible) =
                    existing_regular_files(root, files);
                (
                    existing,
                    InventorySource::FilesystemIncludeUntracked,
                    false,
                    deleted,
                    None,
                    Vec::new(),
                    submods,
                    inaccessible,
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
                    Vec::new(),
                )
            }
        }
    } else {
        match git_ls_files(root) {
            Ok(files) => {
                let empty_git_tracked = files.is_empty();
                let (existing, deleted_tracked, submodule_paths, inaccessible_paths) =
                    existing_regular_files(root, files);
                (
                    existing,
                    InventorySource::GitTracked,
                    empty_git_tracked,
                    deleted_tracked,
                    None,
                    Vec::new(),
                    submodule_paths,
                    inaccessible_paths,
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
                    Vec::new(),
                )
            }
        }
    };
    files.sort();
    files.dedup();
    files.retain(|path| !source_tree_path_is_ignored(path, &options.ignored));
    let completeness = inventory_completeness(
        options,
        git_error.is_some(),
        &deleted_tracked,
        &submodule_paths,
        &inaccessible_paths,
        &skipped_paths,
    );
    Ok(Inventory {
        files,
        source,
        completeness,
        empty_git_tracked,
        deleted_tracked,
        git_error,
        skipped_paths,
        submodule_paths,
        inaccessible_paths,
    })
}

fn inventory_completeness(
    options: &InventoryOptions,
    git_failed: bool,
    deleted_tracked: &[PathBuf],
    submodule_paths: &[PathBuf],
    inaccessible_paths: &[PathBuf],
    skipped_paths: &[PathBuf],
) -> options::InventoryCompleteness {
    if git_failed {
        options::InventoryCompleteness::Fallback
    } else if !deleted_tracked.is_empty()
        || !submodule_paths.is_empty()
        || !inaccessible_paths.is_empty()
        || !skipped_paths.is_empty()
    {
        options::InventoryCompleteness::Partial
    } else if !options.ignored.is_empty() || !options.generated.is_empty() {
        options::InventoryCompleteness::Scoped
    } else {
        options::InventoryCompleteness::Complete
    }
}

#[cfg(test)]
mod tests;
