use allow_inventory::{Inventory, InventoryCompleteness, InventorySource};
use clap::{Args, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Args)]
pub(crate) struct RootArgs {
    /// Source tree root. Defaults to the nearest git root, then current directory.
    #[arg(long)]
    pub(crate) root: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InventoryFacts {
    pub(crate) source: InventorySource,
    pub(crate) completeness: InventoryCompleteness,
    pub(crate) files_scanned: Option<usize>,
    /// True when git inventory succeeded but reported no tracked paths (#1849).
    pub(crate) empty_git_tracked: bool,
    /// Count of git-tracked paths absent from the worktree (deleted-tracked).
    /// Surfaced as an inventory diagnostic so coverage gaps are never silent
    /// (#2048).
    pub(crate) deleted_tracked: Option<usize>,
}

impl InventoryFacts {
    pub(crate) fn source_only(source: InventorySource) -> Self {
        Self {
            source,
            completeness: if source == InventorySource::FilesystemFallback {
                InventoryCompleteness::Fallback
            } else {
                InventoryCompleteness::Partial
            },
            files_scanned: None,
            empty_git_tracked: false,
            deleted_tracked: None,
        }
    }

    pub(crate) fn scanned(source: InventorySource, files_scanned: usize) -> Self {
        Self {
            source,
            completeness: InventoryCompleteness::Scoped,
            files_scanned: Some(files_scanned),
            empty_git_tracked: false,
            deleted_tracked: None,
        }
    }

    pub(crate) fn scanned_inventory(inventory: &Inventory) -> Self {
        Self::scanned(inventory.source, inventory.files.len())
            .with_empty_git_tracked(inventory.empty_git_tracked)
            .with_completeness(inventory.completeness)
    }

    pub(crate) fn with_empty_git_tracked(mut self, empty_git_tracked: bool) -> Self {
        self.empty_git_tracked = empty_git_tracked;
        self
    }

    pub(crate) fn with_deleted_tracked(mut self, deleted_tracked: usize) -> Self {
        self.deleted_tracked = Some(deleted_tracked);
        self
    }

    pub(crate) fn with_completeness(mut self, completeness: InventoryCompleteness) -> Self {
        self.completeness = completeness;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum OutputFormat {
    Human,
    Html,
    Json,
    Sarif,
    #[value(alias = "md")]
    Markdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ProfileArg {
    #[value(name = "spec-system")]
    SpecSystem,
}
