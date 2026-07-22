//! Exact repository source views for the cargo-allow three-product extraction (#2583).
//!
//! This crate owns committed tree, staged index, and working-tree source views for
//! source-tree snapshot consumers. It does not invoke Cargo, compile code, or execute
//! repository policy.

mod git;
mod parity;
mod revision_identity;
mod source_view;
mod source_view_surface;
mod staged_index;

#[cfg(test)]
mod protocol_adapter;

pub use git::{changed_files, git_tracked_files_at_revision, read_file_at_revision};
pub use parity::{ParityContract, load_parity_contract, parity_contract_paths};
pub use revision_identity::RevisionIdentitySurface;
pub use revision_identity::{
    REPOSITORY_SNAPSHOT_SCHEMA, RepositoryDirtyState, RepositoryObjectFormat,
    RepositorySnapshotIdentity, RepositorySnapshotKind, RepositorySnapshotRequest,
    ResolvedRevisionIdentity, SelectedPathIdentity, repository_object_format, repository_snapshot,
    resolve_dirty_state, resolve_revision_identity,
};
pub use source_view::RepositorySourceView;
pub use source_view_surface::SourceViewSurface;
pub use staged_index::StagedIndexSurface;
pub use staged_index::{
    STAGED_GIT_CAPABILITY_GENERATION, StagedEntryKind, StagedGitCapabilities, StagedIndexEntry,
    StagedPathChange, StagedPathRead, StagedPathStatus, StagedRepositorySnapshot,
    StagedSnapshotCompleteness, StagedSnapshotIdentity, probe_staged_git_capabilities,
    read_staged_path, read_staged_raw_path, staged_repository_snapshot,
};

#[cfg(test)]
mod source_view_tests;

#[cfg(test)]
mod tests;
