use std::path::PathBuf;

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

impl From<allow_inventory::InventoryCompleteness> for SourceInventoryCompleteness {
    fn from(value: allow_inventory::InventoryCompleteness) -> Self {
        match value {
            allow_inventory::InventoryCompleteness::Complete => Self::Complete,
            allow_inventory::InventoryCompleteness::Scoped => Self::Scoped,
            allow_inventory::InventoryCompleteness::Fallback => Self::Fallback,
            allow_inventory::InventoryCompleteness::Partial => Self::Partial,
        }
    }
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

impl From<allow_inventory::InventorySource> for SourceInventorySource {
    fn from(value: allow_inventory::InventorySource) -> Self {
        match value {
            allow_inventory::InventorySource::GitTracked => Self::GitTracked,
            allow_inventory::InventorySource::GitIndexStagedCandidate => {
                Self::GitIndexStagedCandidate
            }
            allow_inventory::InventorySource::FilesystemFallback => Self::FilesystemFallback,
            allow_inventory::InventorySource::FilesystemIncludeUntracked => {
                Self::FilesystemIncludeUntracked
            }
        }
    }
}

impl From<allow_inventory::Inventory> for SourceInventory {
    fn from(value: allow_inventory::Inventory) -> Self {
        Self {
            files: value.files,
            source: value.source.into(),
            completeness: value.completeness.into(),
            empty_git_tracked: value.empty_git_tracked,
            deleted_tracked: value.deleted_tracked,
            git_error: value.git_error,
            skipped_paths: value.skipped_paths,
            submodule_paths: value.submodule_paths,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::{SourceInventory, SourceInventoryCompleteness, SourceInventorySource};
    use std::path::PathBuf;

    #[test]
    fn legacy_inventory_conversion_preserves_metadata() {
        let inventory = allow_inventory::Inventory {
            files: vec![PathBuf::from("src/lib.rs")],
            source: allow_inventory::InventorySource::FilesystemFallback,
            completeness: allow_inventory::InventoryCompleteness::Partial,
            empty_git_tracked: true,
            deleted_tracked: vec![PathBuf::from("src/old.rs")],
            git_error: Some("git unavailable".to_string()),
            skipped_paths: vec![PathBuf::from("restricted")],
            submodule_paths: vec![PathBuf::from("vendor/module")],
        };

        let neutral: SourceInventory = inventory.into();

        assert_eq!(neutral.files, [PathBuf::from("src/lib.rs")]);
        assert_eq!(neutral.source, SourceInventorySource::FilesystemFallback);
        assert_eq!(neutral.completeness, SourceInventoryCompleteness::Partial);
        assert!(neutral.empty_git_tracked);
        assert_eq!(neutral.deleted_tracked, [PathBuf::from("src/old.rs")]);
        assert_eq!(neutral.git_error.as_deref(), Some("git unavailable"));
        assert_eq!(neutral.skipped_paths, [PathBuf::from("restricted")]);
        assert_eq!(neutral.submodule_paths, [PathBuf::from("vendor/module")]);
    }

    #[test]
    fn legacy_inventory_enums_and_neutral_labels_are_total() {
        let completeness = [
            (
                allow_inventory::InventoryCompleteness::Complete,
                SourceInventoryCompleteness::Complete,
                "complete",
            ),
            (
                allow_inventory::InventoryCompleteness::Scoped,
                SourceInventoryCompleteness::Scoped,
                "scoped",
            ),
            (
                allow_inventory::InventoryCompleteness::Fallback,
                SourceInventoryCompleteness::Fallback,
                "fallback",
            ),
            (
                allow_inventory::InventoryCompleteness::Partial,
                SourceInventoryCompleteness::Partial,
                "partial",
            ),
        ];
        for (legacy, neutral, label) in completeness {
            assert_eq!(SourceInventoryCompleteness::from(legacy), neutral);
            assert_eq!(neutral.as_str(), label);
        }

        let sources = [
            (
                allow_inventory::InventorySource::GitTracked,
                SourceInventorySource::GitTracked,
                "git_tracked",
            ),
            (
                allow_inventory::InventorySource::GitIndexStagedCandidate,
                SourceInventorySource::GitIndexStagedCandidate,
                "git_index_staged_candidate",
            ),
            (
                allow_inventory::InventorySource::FilesystemFallback,
                SourceInventorySource::FilesystemFallback,
                "filesystem_fallback",
            ),
            (
                allow_inventory::InventorySource::FilesystemIncludeUntracked,
                SourceInventorySource::FilesystemIncludeUntracked,
                "filesystem_include_untracked",
            ),
        ];
        for (legacy, neutral, label) in sources {
            assert_eq!(SourceInventorySource::from(legacy), neutral);
            assert_eq!(neutral.as_str(), label);
        }
    }
}
