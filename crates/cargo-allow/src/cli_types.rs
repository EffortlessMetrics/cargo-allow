use allow_inventory::InventorySource;
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
    pub(crate) files_scanned: Option<usize>,
    /// Count of git-tracked paths absent from the worktree (deleted-tracked).
    /// Surfaced as an inventory diagnostic so coverage gaps are never silent
    /// (#2048).
    pub(crate) deleted_tracked: Option<usize>,
}

impl InventoryFacts {
    pub(crate) fn source_only(source: InventorySource) -> Self {
        Self {
            source,
            files_scanned: None,
            deleted_tracked: None,
        }
    }

    pub(crate) fn scanned(source: InventorySource, files_scanned: usize) -> Self {
        Self {
            source,
            files_scanned: Some(files_scanned),
            deleted_tracked: None,
        }
    }

    pub(crate) fn with_deleted_tracked(mut self, deleted_tracked: usize) -> Self {
        self.deleted_tracked = Some(deleted_tracked);
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
