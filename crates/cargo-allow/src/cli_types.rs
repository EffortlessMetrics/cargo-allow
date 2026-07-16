use allow_core::MatchStatus;
use allow_inventory::{Inventory, InventoryCompleteness, InventorySource};
use clap::{Args, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Args)]
pub(crate) struct RootArgs {
    /// Source tree root. Defaults to the nearest git root, then current directory.
    #[arg(long)]
    pub(crate) root: Option<PathBuf>,
}

/// Parse a `--status` CLI value against the full [`MatchStatus`] vocabulary.
///
/// Kept in one place so list and worklist cannot drift away from scanner statuses
/// such as `location_drift`.
pub(crate) fn parse_match_status_arg(value: &str) -> Result<String, String> {
    if MatchStatus::ALL
        .iter()
        .any(|status| status.as_str() == value)
    {
        return Ok(value.to_string());
    }
    let supported = MatchStatus::ALL
        .iter()
        .map(|status| status.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!(
        "unknown status `{value}`; possible values: {supported}"
    ))
}

#[cfg(test)]
mod match_status_arg_tests {
    use super::*;

    #[test]
    fn parse_match_status_arg_accepts_every_match_status() {
        for status in MatchStatus::ALL {
            let parsed = parse_match_status_arg(status.as_str()).unwrap_or_else(|err| {
                std::panic::panic_any(format!(
                    "status `{}` should be accepted: {err}",
                    status.as_str()
                ))
            });
            assert_eq!(parsed, status.as_str());
        }
    }

    #[test]
    fn parse_match_status_arg_accepts_location_drift() {
        assert_eq!(
            parse_match_status_arg("location_drift").as_deref(),
            Ok("location_drift")
        );
    }

    #[test]
    fn parse_match_status_arg_rejects_unknown() {
        let err = parse_match_status_arg("not_a_status")
            .err()
            .unwrap_or_else(|| std::panic::panic_any("unknown status should fail closed"));
        assert!(err.contains("unknown status `not_a_status`"));
        assert!(err.contains("location_drift"));
    }
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
