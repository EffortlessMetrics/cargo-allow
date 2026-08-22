use crate::error::{SnapshotError, SnapshotErrorKind, SnapshotResult};
use crate::util::source_tree_path_is_ignored;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_IGNORED_PATHS: &[&str] = &[".git/**", "target/**"];
const MAX_DEPTH: usize = 64;
const MAX_ENTRIES: usize = 250_000;

/// Neutral inventory facts shared by repository source-view consumers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInventory {
    /// Repository-relative files included in the source view.
    pub files: Vec<PathBuf>,
    /// How the files were obtained.
    pub source: SourceInventorySource,
    /// Whether the inventory is complete or intentionally limited.
    pub completeness: SourceInventoryCompleteness,
    /// Whether Git reported an empty tracked file set.
    pub empty_git_tracked: bool,
    /// Tracked paths absent from the worktree.
    pub deleted_tracked: Vec<PathBuf>,
    /// Git-listed paths whose metadata probe failed for a reason other than
    /// `NotFound`.
    pub inaccessible_paths: Vec<PathBuf>,
    /// Git error retained when filesystem fallback was used.
    pub git_error: Option<String>,
    /// Paths skipped during filesystem traversal.
    pub skipped_paths: Vec<PathBuf>,
    /// Checked-out submodule paths excluded from recursion.
    pub submodule_paths: Vec<PathBuf>,
}

/// Completeness state for a source inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceInventoryCompleteness {
    Complete,
    Scoped,
    Fallback,
    Partial,
}

impl SourceInventoryCompleteness {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Scoped => "scoped",
            Self::Fallback => "fallback",
            Self::Partial => "partial",
        }
    }
}

/// Source used to produce an inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceInventorySource {
    GitTracked,
    GitIndexStagedCandidate,
    FilesystemFallback,
    FilesystemIncludeUntracked,
}

impl SourceInventorySource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GitTracked => "git_tracked",
            Self::GitIndexStagedCandidate => "git_index_staged_candidate",
            Self::FilesystemFallback => "filesystem_fallback",
            Self::FilesystemIncludeUntracked => "filesystem_include_untracked",
        }
    }
}

pub(crate) fn default_source_inventory(root: &Path) -> SnapshotResult<SourceInventory> {
    let (
        mut files,
        source,
        empty_git_tracked,
        deleted_tracked,
        git_error,
        skipped_paths,
        submodule_paths,
        inaccessible_paths,
    ) = match git_tracked_files(root) {
        Ok(files) => {
            let empty = files.is_empty();
            let (existing, deleted, submodules, inaccessible) = classify_tracked_files(root, files);
            (
                existing,
                SourceInventorySource::GitTracked,
                empty,
                deleted,
                None,
                Vec::new(),
                submodules,
                inaccessible,
            )
        }
        Err(error) => {
            let (files, skipped) = recursive_files(root)?;
            (
                files,
                SourceInventorySource::FilesystemFallback,
                false,
                Vec::new(),
                Some(error),
                skipped,
                Vec::new(),
                Vec::new(),
            )
        }
    };
    files.retain(|path| !source_inventory_path_is_ignored(path));
    files.sort();
    files.dedup();
    let completeness = source_inventory_completeness(
        git_error.is_some(),
        &deleted_tracked,
        &submodule_paths,
        &inaccessible_paths,
        &skipped_paths,
    );
    Ok(SourceInventory {
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

pub(crate) fn source_inventory_path_is_ignored(path: &Path) -> bool {
    source_tree_path_is_ignored(path, DEFAULT_IGNORED_PATHS)
}

fn git_tracked_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z"])
        .output()
        .map_err(|error| format!("failed to invoke git ls-files: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let code = output.status.code().unwrap_or(-1);
        return Err(format!("git ls-files failed (exit {code}): {stderr}"));
    }
    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(bytes_to_path)
        .collect())
}

#[cfg(unix)]
fn bytes_to_path(bytes: &[u8]) -> PathBuf {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    PathBuf::from(OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
fn bytes_to_path(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
}

fn classify_tracked_files(
    root: &Path,
    files: Vec<PathBuf>,
) -> (Vec<PathBuf>, Vec<PathBuf>, Vec<PathBuf>, Vec<PathBuf>) {
    classify_tracked_files_with_metadata(root, files, |path| fs::metadata(path))
}

fn source_inventory_completeness(
    git_failed: bool,
    deleted_tracked: &[PathBuf],
    submodule_paths: &[PathBuf],
    inaccessible_paths: &[PathBuf],
    skipped_paths: &[PathBuf],
) -> SourceInventoryCompleteness {
    if git_failed {
        SourceInventoryCompleteness::Fallback
    } else if !deleted_tracked.is_empty()
        || !submodule_paths.is_empty()
        || !inaccessible_paths.is_empty()
        || !skipped_paths.is_empty()
    {
        SourceInventoryCompleteness::Partial
    } else {
        SourceInventoryCompleteness::Scoped
    }
}

fn classify_tracked_files_with_metadata<F>(
    root: &Path,
    files: Vec<PathBuf>,
    metadata: F,
) -> (Vec<PathBuf>, Vec<PathBuf>, Vec<PathBuf>, Vec<PathBuf>)
where
    F: Fn(&Path) -> std::io::Result<fs::Metadata>,
{
    let mut existing = Vec::new();
    let mut deleted = Vec::new();
    let mut submodules = Vec::new();
    let mut inaccessible = Vec::new();
    for path in files {
        match metadata(&root.join(&path)) {
            Ok(metadata) if metadata.file_type().is_file() => existing.push(path),
            Ok(metadata) if metadata.file_type().is_dir() => submodules.push(path),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => deleted.push(path),
            Err(_) => inaccessible.push(path),
        }
    }
    (existing, deleted, submodules, inaccessible)
}

fn recursive_files(root: &Path) -> SnapshotResult<(Vec<PathBuf>, Vec<PathBuf>)> {
    let mut files = Vec::new();
    let mut skipped = Vec::new();
    visit(root, root, 0, &mut files, &mut skipped)?;
    Ok((files, skipped))
}

fn visit(
    root: &Path,
    directory: &Path,
    depth: usize,
    files: &mut Vec<PathBuf>,
    skipped: &mut Vec<PathBuf>,
) -> SnapshotResult<()> {
    if depth > MAX_DEPTH {
        skipped.push(
            directory
                .strip_prefix(root)
                .unwrap_or(directory)
                .to_path_buf(),
        );
        return Ok(());
    }
    if files.len() >= MAX_ENTRIES {
        skipped.push(PathBuf::from(".source-inventory-entry-limit"));
        return Ok(());
    }
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if directory != root => {
            skipped.push(
                directory
                    .strip_prefix(root)
                    .unwrap_or(directory)
                    .to_path_buf(),
            );
            let _ = error;
            return Ok(());
        }
        Err(error) => {
            return Err(SnapshotError::with_kind(
                SnapshotErrorKind::Inventory,
                format!("failed to read {}: {error}", directory.display()),
            ));
        }
    };
    for entry in entries {
        if files.len() >= MAX_ENTRIES {
            skipped.push(PathBuf::from(".source-inventory-entry-limit"));
            return Ok(());
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                skipped.push(relative_path_for_entry_error(root, directory));
                continue;
            }
        };
        let path = entry.path();
        let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => {
                skipped.push(relative.clone());
                continue;
            }
        };
        if file_type.is_symlink() {
            if fs::metadata(&path)
                .map(|metadata| metadata.file_type().is_file())
                .unwrap_or(false)
            {
                files.push(relative);
            }
        } else if file_type.is_dir() {
            if !source_inventory_path_is_ignored(&relative) {
                visit(root, &path, depth + 1, files, skipped)?;
            }
        } else if file_type.is_file() && !source_inventory_path_is_ignored(&relative) {
            files.push(relative);
        }
    }
    Ok(())
}

fn relative_path_for_entry_error(root: &Path, directory: &Path) -> PathBuf {
    directory
        .strip_prefix(root)
        .unwrap_or(directory)
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::{
        SourceInventoryCompleteness, SourceInventorySource, classify_tracked_files_with_metadata,
        default_source_inventory,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    #[test]
    fn neutral_inventory_labels_are_total() {
        let completeness = [
            (SourceInventoryCompleteness::Complete, "complete"),
            (SourceInventoryCompleteness::Scoped, "scoped"),
            (SourceInventoryCompleteness::Fallback, "fallback"),
            (SourceInventoryCompleteness::Partial, "partial"),
        ];
        for (value, label) in completeness {
            assert_eq!(value.as_str(), label);
        }

        let sources = [
            (SourceInventorySource::GitTracked, "git_tracked"),
            (
                SourceInventorySource::GitIndexStagedCandidate,
                "git_index_staged_candidate",
            ),
            (
                SourceInventorySource::FilesystemFallback,
                "filesystem_fallback",
            ),
            (
                SourceInventorySource::FilesystemIncludeUntracked,
                "filesystem_include_untracked",
            ),
        ];
        for (value, label) in sources {
            assert_eq!(value.as_str(), label);
        }
    }

    #[test]
    fn filesystem_fallback_is_scoped_and_excludes_default_build_paths() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "repo-snapshot-inventory-fallback-{}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("target")).map_err(|error| error.to_string())?;
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"fixture\"\n")
            .map_err(|error| error.to_string())?;
        fs::write(root.join("target/generated.rs"), "ignored")
            .map_err(|error| error.to_string())?;

        let inventory = default_source_inventory(&root).map_err(|error| error.to_string())?;

        assert_eq!(inventory.source, SourceInventorySource::FilesystemFallback);
        assert_eq!(
            inventory.completeness,
            SourceInventoryCompleteness::Fallback
        );
        assert_eq!(inventory.files, [PathBuf::from("Cargo.toml")]);
        assert!(inventory.git_error.is_some());
        fs::remove_dir_all(root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn git_inventory_reports_existing_and_deleted_tracked_paths() -> Result<(), String> {
        let root = temp_root("git-inventory")?;
        git(&root, &["init", "-q"])?;
        git(&root, &["config", "user.name", "Snapshot Tests"])?;
        git(&root, &["config", "user.email", "snapshot@example.invalid"])?;
        fs::write(root.join("kept.rs"), "fn kept() {}\n").map_err(|error| error.to_string())?;
        fs::write(root.join("deleted.rs"), "fn deleted() {}\n")
            .map_err(|error| error.to_string())?;
        git(&root, &["add", "kept.rs", "deleted.rs"])?;
        fs::remove_file(root.join("deleted.rs")).map_err(|error| error.to_string())?;

        let inventory = default_source_inventory(&root).map_err(|error| error.to_string())?;

        assert_eq!(inventory.source, SourceInventorySource::GitTracked);
        assert_eq!(inventory.completeness, SourceInventoryCompleteness::Partial);
        assert_eq!(inventory.files, [PathBuf::from("kept.rs")]);
        assert_eq!(inventory.deleted_tracked, [PathBuf::from("deleted.rs")]);
        fs::remove_dir_all(root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn metadata_errors_are_disclosed_without_acl_dependencies() -> Result<(), String> {
        let root = temp_root("inaccessible-tracked").map_err(|error| error.to_string())?;
        fs::write(root.join("kept.rs"), "fn kept() {}\n").map_err(|error| error.to_string())?;
        let (existing, deleted, submodules, inaccessible) = classify_tracked_files_with_metadata(
            &root,
            vec![
                PathBuf::from("kept.rs"),
                PathBuf::from("deleted.rs"),
                PathBuf::from("blocked.rs"),
            ],
            |path| {
                if path.ends_with("deleted.rs") {
                    Err(std::io::Error::from(std::io::ErrorKind::NotFound))
                } else if path.ends_with("blocked.rs") {
                    Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
                } else {
                    fs::metadata(path)
                }
            },
        );
        assert_eq!(existing, [PathBuf::from("kept.rs")]);
        assert_eq!(deleted, [PathBuf::from("deleted.rs")]);
        assert!(submodules.is_empty());
        assert_eq!(inaccessible, [PathBuf::from("blocked.rs")]);
        fs::remove_dir_all(root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn inaccessible_paths_are_partial_but_git_failure_is_fallback() {
        assert_eq!(
            super::source_inventory_completeness(
                false,
                &[],
                &[],
                &[PathBuf::from("blocked.rs")],
                &[]
            ),
            SourceInventoryCompleteness::Partial
        );
        assert_eq!(
            super::source_inventory_completeness(
                true,
                &[],
                &[],
                &[PathBuf::from("blocked.rs")],
                &[]
            ),
            SourceInventoryCompleteness::Fallback
        );
    }

    #[test]
    fn inventory_reports_missing_root_as_typed_error() -> Result<(), String> {
        let root = temp_root("missing-root")?;
        fs::remove_dir_all(&root).map_err(|error| error.to_string())?;

        let error = default_source_inventory(&root)
            .err()
            .ok_or_else(|| "missing root unexpectedly scanned".to_string())?;

        assert_eq!(error.kind(), super::SnapshotErrorKind::Inventory);
        Ok(())
    }

    #[test]
    fn inventory_walk_limits_record_skips() -> Result<(), String> {
        let root = temp_root("walk-limits")?;
        let mut files = Vec::new();
        let mut skipped = Vec::new();
        super::visit(&root, &root, super::MAX_DEPTH + 1, &mut files, &mut skipped)
            .map_err(|error| error.to_string())?;
        assert_eq!(files.len(), 0);
        assert_eq!(skipped.len(), 1);

        files.resize(super::MAX_ENTRIES, PathBuf::from("existing"));
        skipped.clear();
        super::visit(&root, &root, 0, &mut files, &mut skipped)
            .map_err(|error| error.to_string())?;
        assert_eq!(skipped.len(), 1);
        fs::remove_dir_all(root).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn fallback_inventory_includes_file_symlinks_and_is_sorted() -> Result<(), String> {
        let root = temp_root("fallback-symlink")?;
        fs::create_dir_all(root.join("nested")).map_err(|error| error.to_string())?;
        fs::write(root.join("nested/z.rs"), "z\n").map_err(|error| error.to_string())?;
        fs::write(root.join("a.rs"), "a\n").map_err(|error| error.to_string())?;
        std::os::unix::fs::symlink("nested/z.rs", root.join("alias.rs"))
            .map_err(|error| error.to_string())?;

        let inventory = default_source_inventory(&root).map_err(|error| error.to_string())?;

        assert_eq!(inventory.source, SourceInventorySource::FilesystemFallback);
        assert_eq!(
            inventory.files,
            [
                PathBuf::from("a.rs"),
                PathBuf::from("alias.rs"),
                PathBuf::from("nested/z.rs"),
            ]
        );
        fs::remove_dir_all(root).map_err(|error| error.to_string())?;
        Ok(())
    }

    fn git(root: &Path, args: &[&str]) -> Result<(), String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }

    fn temp_root(label: &str) -> Result<PathBuf, String> {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "repo-snapshot-inventory-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        Ok(root)
    }
}
