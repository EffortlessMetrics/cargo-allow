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
