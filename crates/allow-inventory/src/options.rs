use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct InventoryOptions {
    pub ignored: Vec<String>,
    pub generated: Vec<String>,
    pub include_untracked: bool,
}

impl Default for InventoryOptions {
    fn default() -> Self {
        Self {
            ignored: vec![".git/**".to_string(), "target/**".to_string()],
            generated: Vec::new(),
            include_untracked: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventorySource {
    GitTracked,
    GitIndexStagedCandidate,
    FilesystemFallback,
    FilesystemIncludeUntracked,
}

/// Whether an inventory is complete, intentionally scoped, obtained through
/// fallback, or missing material paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryCompleteness {
    Complete,
    Scoped,
    Fallback,
    Partial,
}

impl InventoryCompleteness {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Scoped => "scoped",
            Self::Fallback => "fallback",
            Self::Partial => "partial",
        }
    }
}

impl InventorySource {
    pub const ALL: &[Self] = &[
        Self::GitTracked,
        Self::GitIndexStagedCandidate,
        Self::FilesystemFallback,
        Self::FilesystemIncludeUntracked,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::GitTracked => "git_tracked",
            Self::GitIndexStagedCandidate => "git_index_staged_candidate",
            Self::FilesystemFallback => "filesystem_fallback",
            Self::FilesystemIncludeUntracked => "filesystem_include_untracked",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inventory {
    pub files: Vec<PathBuf>,
    pub source: InventorySource,
    pub completeness: InventoryCompleteness,
    /// Git reported an empty tracked file set for a git-tracked inventory.
    /// This distinguishes a successful non-empty tracked scan from the fresh
    /// `git init` case where no files were scanned (#1849).
    pub empty_git_tracked: bool,
    /// Git-tracked paths that are absent from the worktree (deleted-tracked).
    /// Reported as an inventory diagnostic so a scan never looks complete while
    /// a tracked path disappeared from coverage (#2048). Empty for non-git
    /// inventory sources.
    pub deleted_tracked: Vec<PathBuf>,
    /// Git-listed paths whose metadata probe failed for a reason other than
    /// `NotFound`.
    pub inaccessible_paths: Vec<PathBuf>,
    /// When the git inventory source failed and the scan fell back to the
    /// filesystem, this carries the git error message so the fallback is never
    /// silent (#1845). `None` when git succeeded or was not attempted.
    pub git_error: Option<String>,
    /// Directories/files skipped during filesystem traversal due to permission
    /// errors or other I/O failures (#1844). A single unreadable sub-directory
    /// no longer aborts the entire walk; it is recorded here and the rest of
    /// the tree is scanned. Empty for git-tracked inventory.
    pub skipped_paths: Vec<PathBuf>,
    /// Git-tracked paths that are directories on disk (checked-out submodules).
    /// Their contents are not scanned (submodule recursion is out of scope) but
    /// the paths are surfaced so the exclusion is never silent (#1846).
    pub submodule_paths: Vec<PathBuf>,
}

#[cfg(test)]
#[path = "options_tests.rs"]
mod tests;
